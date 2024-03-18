FROM ubuntu:jammy-20230301
#RUN apt-get update &&\
#    apt-get install -y heaptrack &&\
#    apt-get install -y heaptrack-gui
# RUN apt-get update &&\
#    apt -y install pkg-config libfreetype6-dev libfontconfig1-dev mingw-w64
COPY kt /tmp/kt
RUN chmod go+r /tmp/kt
WORKDIR "/tmp"
# ENTRYPOINT ["/tmp/kt"]
ENTRYPOINT ["tail", "-f", "/dev/null"]

