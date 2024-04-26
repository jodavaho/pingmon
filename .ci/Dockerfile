from ubuntu:latest
# install the deb we built
COPY ./pingmon*.deb /tmp/
RUN apt-get install -y /tmp/*.deb
RUN cat /etc/pingmon.toml
RUN cat /usr/share/doc/pingmon/README.md
RUN /usr/bin/pingmon -h
