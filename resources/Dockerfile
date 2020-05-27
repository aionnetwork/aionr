FROM ubuntu:18.04
ARG PACKAGE_LOCATION

RUN mkdir aionr
RUN apt update
RUN apt -y install lsb-release wget bzip2 gawk libboost-filesystem1.65-dev libboost-program-options1.65-dev llvm-4.0-dev locales

WORKDIR /aionr
ADD $PACKAGE_LOCATION /aionr
ADD libs /run/libs

ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV JAVA_HOME="/run/libs/jdk-11.0.1"
ENV LIBRARY_PATH="${JAVA_HOME}/lib/server"
ENV LD_LIBRARY_PATH="${LIBRARY_PATH}:/usr/local/lib:/run/libs" PATH="${PATH}:${JAVA_HOME}/bin"

ENTRYPOINT ["/bin/bash"]
CMD ["./mainnet.sh", "--base-path=mainnet/base"]

EXPOSE 30303 8545 8546 8547 8008