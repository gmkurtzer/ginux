// SPDX-License-Identifier: GPL-2.0-only

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static int denied(const char *path)
{
	errno = 0;
	return syscall(SYS_mkdirat, AT_FDCWD, path, 0700) == -1 &&
	       errno == EPERM;
}

static void *thread_main(void *unused)
{
	(void)unused;
	return (void *)(long)!denied("/tmp/ginux-thread-raw");
}

int main(int argc, char **argv)
{
	pthread_t thread;
	void *thread_result;
	pid_t child;
	int status;

	if (argc != 2) {
		fprintf(stderr, "usage: %s STATIC_RAW_FIXTURE\n", argv[0]);
		return 2;
	}
	if (!denied("/tmp/ginux-parent-raw"))
		return 1;
	if (pthread_create(&thread, NULL, thread_main, NULL) != 0 ||
	    pthread_join(thread, &thread_result) != 0 || thread_result != NULL)
		return 1;

	child = fork();
	if (child < 0)
		return 1;
	if (child == 0)
		_exit(denied("/tmp/ginux-child-raw") ? 0 : 1);
	if (waitpid(child, &status, 0) != child || !WIFEXITED(status) ||
	    WEXITSTATUS(status) != 0)
		return 1;

	child = fork();
	if (child < 0)
		return 1;
	if (child == 0) {
		execl(argv[1], argv[1], NULL);
		_exit(127);
	}
	if (waitpid(child, &status, 0) != child || !WIFEXITED(status) ||
	    WEXITSTATUS(status) != 0)
		return 1;

	puts("GINUX_TRANSITIONS: parent=denied thread=denied fork=denied exec=denied");
	return 0;
}
