// SPDX-License-Identifier: GPL-2.0-only
/*
 * Gate 16 feasibility prototype. This is not yet a supported Ginux backend.
 *
 * The prototype attaches one policy to a cgroup v2 subtree and denies mkdirat
 * before the VFS operation. Unrelated syscalls have no Ginux filter installed.
 */

#include <linux/cgroup.h>
#include <linux/errno.h>
#include <linux/fs.h>
#include <linux/kprobes.h>
#include <linux/miscdevice.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/rcupdate.h>
#include <linux/sched.h>
#include <linux/uaccess.h>

#include "ginux_control.h"

struct ginux_policy {
	struct cgroup *cgroup;
};

static DEFINE_MUTEX(policy_lock);
static struct ginux_policy __rcu *active_policy;

static noinline long ginux_deny_mkdirat(const struct pt_regs *regs)
{
	(void)regs;
	return -EPERM;
}

static int ginux_mkdirat_pre(struct kprobe *probe, struct pt_regs *regs)
{
	struct ginux_policy *policy;
	bool deny = false;

	(void)probe;
	rcu_read_lock();
	policy = rcu_dereference(active_policy);
	if (policy != NULL)
		deny = cgroup_is_descendant(task_dfl_cgroup(current),
					    policy->cgroup);
	rcu_read_unlock();

	if (!deny)
		return 0;

	instruction_pointer_set(regs, (unsigned long)ginux_deny_mkdirat);
	return 1;
}

static struct kprobe mkdirat_probe = {
	.symbol_name = "__x64_sys_mkdirat",
	.pre_handler = ginux_mkdirat_pre,
};

static int ginux_set_cgroup(int fd)
{
	struct ginux_policy *new_policy;
	struct ginux_policy *old_policy;
	struct cgroup *cgroup;

	cgroup = cgroup_get_from_fd(fd);
	if (IS_ERR(cgroup))
		return PTR_ERR(cgroup);

	new_policy = kzalloc(sizeof(*new_policy), GFP_KERNEL);
	if (new_policy == NULL) {
		cgroup_put(cgroup);
		return -ENOMEM;
	}
	new_policy->cgroup = cgroup;

	mutex_lock(&policy_lock);
	old_policy = rcu_dereference_protected(active_policy,
						lockdep_is_held(&policy_lock));
	if (old_policy == NULL && !try_module_get(THIS_MODULE)) {
		mutex_unlock(&policy_lock);
		cgroup_put(cgroup);
		kfree(new_policy);
		return -ENODEV;
	}
	rcu_assign_pointer(active_policy, new_policy);
	mutex_unlock(&policy_lock);

	synchronize_rcu();
	if (old_policy != NULL) {
		cgroup_put(old_policy->cgroup);
		kfree(old_policy);
	}
	return 0;
}

static int ginux_clear_policy(void)
{
	struct ginux_policy *old_policy;

	mutex_lock(&policy_lock);
	old_policy = rcu_dereference_protected(active_policy,
						lockdep_is_held(&policy_lock));
	if (old_policy == NULL) {
		mutex_unlock(&policy_lock);
		return -ENOENT;
	}
	RCU_INIT_POINTER(active_policy, NULL);
	mutex_unlock(&policy_lock);

	synchronize_rcu();
	cgroup_put(old_policy->cgroup);
	kfree(old_policy);
	module_put(THIS_MODULE);
	return 0;
}

static long ginux_control_ioctl(struct file *file, unsigned int command,
				unsigned long argument)
{
	int cgroup_fd;
	unsigned int abi = GINUX_CONTROL_ABI;

	(void)file;
	switch (command) {
	case GINUX_CONTROL_SET_CGROUP:
		if (copy_from_user(&cgroup_fd, (void __user *)argument,
				   sizeof(cgroup_fd)) != 0)
			return -EFAULT;
		return ginux_set_cgroup(cgroup_fd);
	case GINUX_CONTROL_CLEAR:
		return ginux_clear_policy();
	case GINUX_CONTROL_GET_ABI:
		if (copy_to_user((void __user *)argument, &abi, sizeof(abi)) != 0)
			return -EFAULT;
		return 0;
	default:
		return -ENOTTY;
	}
}

static const struct file_operations ginux_control_fops = {
	.owner = THIS_MODULE,
	.unlocked_ioctl = ginux_control_ioctl,
#ifdef CONFIG_COMPAT
	.compat_ioctl = ginux_control_ioctl,
#endif
};

static struct miscdevice ginux_control_device = {
	.minor = MISC_DYNAMIC_MINOR,
	.name = "ginux-control",
	.fops = &ginux_control_fops,
	.mode = 0600,
};

static int __init ginux_control_init(void)
{
	int result;

	result = register_kprobe(&mkdirat_probe);
	if (result != 0)
		return result;

	result = misc_register(&ginux_control_device);
	if (result != 0) {
		unregister_kprobe(&mkdirat_probe);
		return result;
	}

	pr_info("ginux-control: ABI %u loaded\n", GINUX_CONTROL_ABI);
	return 0;
}

static void __exit ginux_control_exit(void)
{
	misc_deregister(&ginux_control_device);
	unregister_kprobe(&mkdirat_probe);
	pr_info("ginux-control: unloaded\n");
}

module_init(ginux_control_init);
module_exit(ginux_control_exit);

MODULE_DESCRIPTION("Ginux Gate 16 kernel-assisted enforcement prototype");
MODULE_AUTHOR("Ginux contributors");
MODULE_LICENSE("GPL");
MODULE_VERSION("0.1");
