# Copyright 2023 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0

# pylint: disable=too-many-lines
import json
import logging
import random
import string
import os
import time
import pytest
from framework import decorators
from framework.artifacts import DEFAULT_DEV_NAME, NetIfaceConfig, ArtifactCollection
from framework.builder import MicrovmBuilder, SnapshotBuilder, SnapshotType
from framework.utils import (
    generate_mmds_session_token,
    configure_mmds,
    generate_mmds_get_request,
    get_firecracker_version_from_toml,
    compare_versions,
)
from framework.utils_vsock import (
    make_blob,
    check_host_connections,
    check_guest_connections,
    check_vsock_device,
    _copy_vsock_data_to_guest,
    make_host_port_path,
    close_guest_connections,
    HostEchoWorker,
    ECHO_SERVER_PORT,
    VSOCK_UDS_PATH,
)
from conftest import _test_images_s3_bucket
import traceback
import sys


import host_tools.drive as drive_tools
import host_tools.network as net_tools
import host_tools.logging as log_tools

# Minimum lifetime of token.
MIN_TOKEN_TTL_SECONDS = 1
# Maximum lifetime of token.
MAX_TOKEN_TTL_SECONDS = 21600
# Default IPv4 value for MMDS.
DEFAULT_IPV4 = "169.254.169.254"
# MMDS versions supported.
# MMDS_VERSIONS = ["V2", "V1"]
MMDS_VER = "V2"

NO_OF_MICROVMS = 20


def _configure_and_run(
    test_microvm_with_api,
    network_info
):
    """Auxiliary function for configuring and running microVM."""

    test_microvm = test_microvm_with_api
    # Set a large enough limit for the API so that requests actually reach the
    # MMDS server.
    test_microvm.jailer.extra_args.update(
        {"http-api-max-payload-size": "512000", "mmds-size-limit": "51200"}
    )
    test_microvm.spawn(create_logger=False)

    # Configure logging.
    log_fifo_path = os.path.join(test_microvm.path, "log_fifo")
    log_fifo = log_tools.Fifo(log_fifo_path)

    print(f"{log_fifo.path=}")
    response = test_microvm.logger.put(
        log_path=test_microvm.create_jailed_resource(log_fifo.path),
        level="Debug",
        show_level=True,
        show_log_origin=True,
    )
    assert test_microvm.api_session.is_status_no_content(response.status_code)
    test_microvm.start_console_logger(log_fifo)

    # "cpu_template": "T2",
    config = {"vcpu_count": 2, "mem_size_mib": 2630, "smt": True}

    test_microvm.basic_config(**config)

    rlim = {"bandwidth": {"refill_time": 1000, "size":125000000}}
    _tap, _, guest_ip = test_microvm.ssh_network_config(
        network_info["config"], network_info["iface_id"],
        rx_rate_limiter=rlim, tx_rate_limiter=rlim
    )

    test_microvm.vsock.put(vsock_id="vsock0", guest_cid=3, uds_path="/{}".format(VSOCK_UDS_PATH))

    configure_mmds(test_microvm, iface_ids=[network_info["iface_id"]], version=MMDS_VER)

    dummy_json = {"latest": {"meta-data": {"ami-id": "dummy"}}}

    # Populate data-store.
    response = test_microvm.mmds.put(json=dummy_json)
    assert test_microvm.api_session.is_status_no_content(response.status_code)

    # Add RW drives.
    for i in range(0, 5):
        fsid = "scratch{}".format(i)
        fs = drive_tools.FilesystemFile(os.path.join(test_microvm.fsfiles, fsid))
        # TODO test with Async too
        test_microvm.add_drive(fsid, fs.path, io_engine="Sync")

        # TODO rate limit one of them
        # if i == 0:
        #     # Rate limit one of them.
        #     test_microvm.add_drive(
        #         fsid, fs.path, io_engine="Sync",
        #         rate_limiter={"ops": {"size": 7500, "refill_time": 1000}}
        #     )
        # else:
        #     test_microvm.add_drive(fsid, fs.path, io_engine="Sync")

    test_microvm.start()


def _run_vsock(vm, ssh_connection, test_fc_session_root_path, bin_vsock_path, i):
    vm_blob_path = "/tmp/vsock/test.blob"

    # Generate a random data file for vsock.
    print('trying to make_blob {}'.format(test_fc_session_root_path))
    blob_path, blob_hash = make_blob(test_fc_session_root_path)
    print('make_blob returned {}'.format(blob_path))

    # Copy the data file and a vsock helper to the guest.
    _copy_vsock_data_to_guest(ssh_connection, blob_path, vm_blob_path, bin_vsock_path)
    print('copied {} and {} to guest'.format(blob_path, bin_vsock_path))

    # Test vsock guest-initiated connections.
    path = os.path.join(vm.path, make_host_port_path(VSOCK_UDS_PATH, ECHO_SERVER_PORT))
    print('host srv port path {}'.format(path))
    check_guest_connections(vm, path, vm_blob_path, blob_hash, i)

    # Test vsock host-initiated connections.
    # path = os.path.join(vm.jailer.chroot_path(), VSOCK_UDS_PATH)
    # check_host_connections(vm, path, blob_path, blob_hash)



def _patch_mmds(test_microvm_with_api):
    """Auxiliary function that issues a PATCH /mmds request."""

    test_microvm = test_microvm_with_api

    # Send a request that will fill the data store.
    aux = "a" * 1000
    # aux = "a" * 663
    dummy_json = {"latest": {"meta-data": {"ami-id": "smth", "secret_key": aux}}}
    # dummy_json = {"task":{"Cluster":"arn:aws:ecs:eu-west-3:292444539842:cluster/moontide-cluster","TaskARN":"arn:aws:ecs:eu-west-3:292444539842:task/moontide-cluster/19a3e5a469ab4f25a601c2a5f2799c31","Family":"taskdef-ff2c45b85cc2daaade2e4555abe4d9618e2ff0b78ba4eb5ac950ca7d8c2eb4a0","Revision":"1","DesiredStatus":"RUNNING","KnownStatus":"SNAPSHOTTER_SELECTED","Limits":{"CPU":1,"Memory":2048},"PullStartedAt":"2022-12-13T13:51:53.540651157Z","PullStoppedAt":"2022-12-13T13:52:07.475167644Z","AvailabilityZone":"eu-west-3a","LaunchType":"FARGATE","ClockDrift":{"ClockErrorBound":0.390797,"ReferenceTimestamp":"2022-12-13T13:52:31Z","ClockSynchronizationStatus":"SYNCHRONIZED"}}}

    # print(f"{dummy_json=}")
    # start = time.time()
    response = test_microvm.mmds.patch(json=dummy_json)
    # try:
    #     response = test_microvm.mmds.patch(json=dummy_json)
    # except Exception as e:
    #     print(e)
    #     # close_guest_connections()
    #     # end = time.time()
    #     # print(f"patch MMD took time {end-start}secs")
    #     # assert f"patch MMD took time {end-start}secs"
    #     # traceback.print_exception(*sys.exc_info())
    #     # traceback.print_exc()
    #     pass

    # end = time.time()
    # if (end-start) > 2:
    #     assert f"patch MMD took time {end-start}secs"
    # # print(f"{response}")
    # assert test_microvm.api_session.is_status_no_content(response.status_code)

def _run_fio(ssh_connection):
    cmd = """nohup fio --filename=/dev/vdb --direct=1 --rw=randrw --bs=64k \
        --ioengine=libaio --iodepth=64 --runtime=120 --numjobs=25 --time_based \
        --group_reporting --name=throughput-test-job --eta-newline=1 &"""
    return ssh_connection.execute_command_bg(cmd)


@pytest.mark.timeout(120000)
@decorators.test_context("api", NO_OF_MICROVMS)
def test_20vms_mmds_vsock(
    test_multiple_microvms,
    network_config,
    bin_vsock_path,
    test_fc_session_root_path
):
    """
    Check we can spawn multiple microvms.

    @type: functional
    """
    # print(locals())
    microvms = test_multiple_microvms
    ssh_conns = []

    for i in range(NO_OF_MICROVMS):
        microvm = microvms[i]
        _configure_and_run(
            microvm,
            {"config": network_config, "iface_id": str(i)}
        )
        # We check that the vm is running by testing that the ssh does
        # not time out.
        ssh_conns.append(net_tools.SSHConnection(microvm.ssh_config))
        print(f"vm {i} configured and run")

    # for i in range(NO_OF_MICROVMS):
    #     _run_fio(ssh_conns[i])

    try:
        for j in range(1000):
        # while True:
            for i in range(NO_OF_MICROVMS):
                print(f"vm {j}_{i} ")
                # print(f'_run_vsock for microvm {i}')
                _run_vsock(microvms[i], ssh_conns[i], test_fc_session_root_path, bin_vsock_path, i)
            for i in range(NO_OF_MICROVMS):
                # print(f'_patch_mmds for microvm {i}')
                _patch_mmds(microvms[i])

            # for i in range(NO_OF_MICROVMS):

            # time.sleep(5)
            # time.sleep(10)
    except Exception as e:
        # for i in range(NO_OF_MICROVMS):
        #     print(str(microvms[i].log_data))
        # pass
        print(f"============<><><><>============={e}")
        # for i in range(NO_OF_MICROVMS):
        #     microvms[i].kill()
        traceback.print_exc()
        # traceback.print_exception(*sys.exc_info())
    finally:
        for i in range(NO_OF_MICROVMS):
            print(f"===============>Log of microvm {i}")
            print(str(microvms[i].log_data))
        close_guest_connections()
        for i in range(NO_OF_MICROVMS):
            microvms[i].kill()

    # print(str(microvms[0].log_data))
    print("====The End====")
