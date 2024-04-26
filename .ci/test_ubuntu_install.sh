cargo deb 
cp target/debian/*.deb .ci/ 
pushd .ci
docker build --no-cache -t test_ubuntu_install .
popd
