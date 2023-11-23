version=$1
echo $version

TAP_DEV="tap0"
TAP_IP="192.168.0.1"
MASK_SHORT="/30"
NAME="firetest"
ip netns del "$NAME" 2> /dev/null || true
ip netns add "$NAME"
ip netns exec "$NAME" ip tuntap add dev "$TAP_DEV" mode tap
ip netns exec "$NAME" ip addr add "${TAP_IP}${MASK_SHORT}" dev "$TAP_DEV"
ip netns exec "$NAME" ip link set dev "$TAP_DEV" up
useradd firecracker1
rm -rf   /srv/jailer/firecracker_$version/firetest/root
mkdir -p /srv/jailer/firecracker_$version/firetest/root
cp run/* /srv/jailer/firecracker_$version/firetest/root/
touch /srv/jailer/firecracker_$version/firetest/root/fc.ndjson
touch /srv/jailer/firecracker_$version/firetest/root/fc.log
chown $(id -g firecracker1):$(id -u firecracker1) /srv/jailer/firecracker_$version/firetest/root/fc.ndjson
chown $(id -g firecracker1):$(id -u firecracker1) /srv/jailer/firecracker_$version/firetest/root/fc.log
./jailer_$version --version
echo $$
echo ./jailer_$version --id $NAME --exec-file ./firecracker_$version --uid $(id -u firecracker1) --gid $(id -g firecracker1) --netns "/var/run/netns/$NAME" --daemonize --new-pid-ns -- --config-file "config.json"  --metrics-path fc.ndjson --log-path fc.log --level Debug 
# echo $?
echo ps -aux | grep fire
echo tail -f /srv/jailer/firecracker_$version/firetest/root/fc.ndjson
#./jailer --id $NAME --exec-file ./firecracker_debug --uid $(id -u firecracker1) --gid $(id -g firecracker1) --netns "/var/run/netns/$NAME" --new-pid-ns -- --config-file "config.json"  --metrics-path fc.ndjson
#./jailer --id $NAME --exec-file ./firecracker_debug --uid $(id -u firecracker1) --gid $(id -g firecracker1) --netns "/var/run/netns/$NAME" --daemonize --new-pid-ns -- --config-file "config.json"  --metrics-path fc.ndjson

# jailer
#  --id 5f5e5ff2-8ece-4228-ad21-18908dd9e535
#  --exec-file firecracker
#  --uid 1234
#  --gid 1234
#  --chroot-base-dir /srv/jailer
#  --netns /var/run/netns/5f5e5ff2-8ece-4228-ad21-18908dd9e535
#  --daemonize
#  --new-pid-ns
#  -- --log-path fc.log --level Debug --metrics-path fc.ndjson

# ./jailer
#  --id uvm000k2
# --exec-file ./firecracker
# --uid 1000722
# --gid 1000722
# --chroot-base-dir /opt/kma/var/lib/uvm/jail
# --netns /var/run/netns/uvm000k2
# --daemonize
# --new-pid-ns

# jailer --id firetest --exec-file firecracker --uid 1234 --gid 1234 --chroot-base-dir /srv/jailer --netns /var/run/netns/firetest --daemonize --new-pid-ns

# /firecracker/build/cargo_target/aarch64-unknown-linux-musl/release/jailer