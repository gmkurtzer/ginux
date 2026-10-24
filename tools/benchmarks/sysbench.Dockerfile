FROM rockylinux/rockylinux:10

RUN dnf -y install epel-release \
    && dnf -y install sysbench \
    && dnf clean all

CMD ["/bin/sh"]
