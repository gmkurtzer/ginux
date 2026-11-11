// SPDX-License-Identifier: GPL-2.0-only

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "ginux_control.h"

static void fail(const char *message)
{
	perror(message);
	exit(1);
}

int main(int argc, char **argv)
{
	int control;
	int cgroup;
	unsigned int abi = 0;

	if (argc != 2) {
		fprintf(stderr, "usage: %s CGROUP|--clear\n", argv[0]);
		return 2;
	}

	control = open(GINUX_CONTROL_DEVICE, O_RDONLY | O_CLOEXEC);
	if (control < 0)
		fail("open control");
	if (ioctl(control, GINUX_CONTROL_GET_ABI, &abi) != 0)
		fail("get ABI");
	if (abi != GINUX_CONTROL_ABI) {
		fprintf(stderr, "unexpected ABI %u\n", abi);
		return 1;
	}
	if (strcmp(argv[1], "--clear") == 0) {
		if (ioctl(control, GINUX_CONTROL_CLEAR) != 0)
			fail("clear policy");
		puts("GINUX_CONTROL: policy=cleared");
		return 0;
	}

	cgroup = open(argv[1], O_RDONLY | O_DIRECTORY | O_CLOEXEC);
	if (cgroup < 0)
		fail("open cgroup");
	if (ioctl(control, GINUX_CONTROL_SET_CGROUP, &cgroup) != 0)
		fail("set cgroup");

	printf("GINUX_CONTROL: abi=%u cgroup=%s policy=mkdirat:EPERM\n",
	       abi, argv[1]);
	return 0;
}
