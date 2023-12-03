#!/usr/bin/env python3
# Copyright 2022 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0

"""Generate Buildkite pipelines dynamically"""

from common import (COMMON_PARSER, get_changed_files, group, overlay_dict,
                    pipeline_to_json, run_all_tests)

# Buildkite default job priority is 0. Setting this to 1 prioritizes PRs over
# scheduled jobs and other batch jobs.
DEFAULT_PRIORITY = 1


args = COMMON_PARSER.parse_args()

defaults = {
    "instances": args.instances,
    "platforms": args.platforms,
    # buildkite step parameters
    "priority": DEFAULT_PRIORITY,
    "timeout_in_minutes": 45,
    "artifacts": ["./test_results/**/*"],
}
defaults = overlay_dict(defaults, args.step_param)

defaults_once_per_architecture = defaults.copy()
defaults_once_per_architecture["instances"] = ["m5d.metal", "c7g.metal"]
defaults_once_per_architecture["platforms"] = [("al2", "linux_5.10")]

functional_grp = group(
    "⚙ Functional and security 🔒",
    "./tools/devtool -y test -- -n 8 --dist worksteal integration_tests/performance/test_vhost_user_metrics.py::test_vhost_user_block_metrics",
    **defaults,
)

defaults_for_performance = overlay_dict(
    defaults,
    {
        # We specify higher priority so the ag=1 jobs get picked up before the ag=n
        # jobs in ag=1 agents
        "priority": DEFAULT_PRIORITY + 1,
        "agents": {"ag": 1},
    },
)

performance_grp = group(
    "⏱ Performance",
    "./tools/devtool -y test --performance -c 1-10 -m 0 -- ../tests/integration_tests/performance/",
    **defaults_for_performance,
)


steps = [functional_grp]

pipeline = {"steps": steps}
print(pipeline_to_json(pipeline))
