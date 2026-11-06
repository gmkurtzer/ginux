/* Native end-to-end workloads. Each CASE selects an independent native scenario. */
typedef unsigned long word;
static long call3(long n, word a, word b, word c) {
    long r;
    register word r10 __asm__("r10")=0, r8 __asm__("r8")=0, r9 __asm__("r9")=0;
    __asm__ volatile("syscall" : "=a"(r) : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9) : "rcx","r11","memory");
    return r;
}
static long __attribute__((unused)) call4(long n, word a, word b, word c, word d) {
    long r;
    register word r10 __asm__("r10")=d, r8 __asm__("r8")=0, r9 __asm__("r9")=0;
    __asm__ volatile("syscall" : "=a"(r) : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9) : "rcx","r11","memory");
    return r;
}
static void show(const char *label, long value) {
    char buffer[128], digits[32];
    word n=0, count=0;
    while(label[n]) { buffer[n]=label[n]; n++; }
    unsigned long u=value<0?-value:value;
    if(value<0)buffer[n++]='-';
    do { digits[count++]='0'+u%10;u/=10; }while(u);
    while(count)buffer[n++]=digits[--count];
    buffer[n++]='\n';
    call3(1,1,(word)buffer,n);
}
static volatile int done;
static int descriptors[2];
static char child_stack[16384] __attribute__((aligned(16)));
static int alive;
static int parent_tid, child_tid;
static char exec_path[1024];
#if defined(CASE_concurrent_wait) || defined(CASE_remaining_wait)
static volatile int ready, completed;
static long selected_child;
static char second_stack[16384] __attribute__((aligned(16)));
#endif
extern long make_thread(word flags, void *stack, word ptid, word ctid);
void thread_body(void) {
#if defined(CASE_concurrent_wait) || defined(CASE_remaining_wait)
    int status=0x12345678;
    __atomic_add_fetch(&ready,1,__ATOMIC_SEQ_CST);
    long result=call3(61,selected_child,(word)&status,0);
    if(result==-10 && status!=0x12345678)call3(231,20,0,0);
    show("concurrent wait result=",result);
    __atomic_add_fetch(&completed,1,__ATOMIC_SEQ_CST);
#elif defined(CASE_thread_reuse)
    __atomic_store_n(&done,1,__ATOMIC_RELEASE);
#elif defined(CASE_thread_exec) || defined(CASE_exec_failure)
    char *argv[]={exec_path,0};
    show("thread exec result=",call3(59,(word)exec_path,(word)argv,0));
    __atomic_store_n(&done,1,__ATOMIC_RELEASE);
#elif defined(CASE_clone_tid)
    show("child tid cell=",child_tid);
    show("child gettid=",call3(186,0,0,0));
#elif defined(CASE_copied_files)
    show("child close=",call3(3,descriptors[1],0,0));
    __atomic_store_n(&done,1,__ATOMIC_RELEASE);
#elif defined(CASE_thread_segv)
    // The fault kills the whole group while the main thread is in syscalls.
    static int *volatile null_pointer;
    *null_pointer=1;
#elif defined(CASE_thread_term)
    __atomic_store_n(&done,1,__ATOMIC_RELEASE);
    for(;;)call3(186,0,0,0);
#elif defined(CASE_parked_kill)
    // Release the grandchild and fault at once: its death races the group
    // death of the parked waiter (this thread's main thread).
    for(int i=0;i<(int)(call3(39,0,0,0)&3);i++)call3(186,0,0,0);
    call3(1,descriptors[1],(word)"x",1);
    static int *volatile null_pointer;
    *null_pointer=1;
#else
    long p=call3(22,(word)descriptors,0,0);
    show("child pipe result=",p);
    call3(1,descriptors[1],(word)"x",1);
    __atomic_store_n(&done,1,__ATOMIC_RELEASE);
#endif
}
__asm__(".global make_thread\nmake_thread:\n"
        "mov %rcx,%r10\nmov $56,%rax\nsyscall\ntest %rax,%rax\njnz 1f\n"
        "call thread_body\nxor %rdi,%rdi\nmov $60,%rax\nsyscall\n1: ret\n");
long probe_main(word *stack) {
    const char *self=((char **)(stack+1))[0];
    word last=0, i=0;
    while(self[i] && i<sizeof(exec_path)-32) {
        exec_path[i]=self[i];
        if(self[i]=='/')last=i+1;
        i++;
    }
#if defined(CASE_exec_failure)
    const char suffix[]="missing-exec-target";
#else
    const char suffix[]="process-exec-target";
#endif
    for(word j=0;j<sizeof suffix;j++)exec_path[last+j]=suffix[j];
#if defined(CASE_concurrent_wait) || defined(CASE_remaining_wait)
    if(call3(22,(word)descriptors,0,0))return 10;
    selected_child=call3(57,0,0,0);
    if(!selected_child) {
        char byte;
        call3(0,descriptors[0],(word)&byte,1);
        return 0;
    }
    if(selected_child<0)return 11;
#if defined(CASE_remaining_wait)
    // Any-child waiters must keep waiting while another child is alive.
    long second=call3(57,0,0,0);
    if(!second) {
        char byte;
        call3(0,descriptors[0],(word)&byte,1);
        return 0;
    }
    if(second<0)return 12;
    selected_child=-1;
#endif
    if(make_thread(0x10f00,child_stack+sizeof child_stack,0,0)<0)return 13;
    if(make_thread(0x10f00,second_stack+sizeof second_stack,0,0)<0)return 14;
    while(__atomic_load_n(&ready,__ATOMIC_SEQ_CST)<2)call3(186,0,0,0);
    // Give both workers time to enter wait before releasing a child.
    for(int i=0;i<100;i++)call3(186,0,0,0);
    call3(1,descriptors[1],(word)"x",1);
#if defined(CASE_remaining_wait)
    while(__atomic_load_n(&completed,__ATOMIC_SEQ_CST)<1)call3(186,0,0,0);
    for(int i=0;i<100;i++)call3(186,0,0,0);
    if(__atomic_load_n(&completed,__ATOMIC_SEQ_CST)!=1)return 15;
    call3(1,descriptors[1],(word)"x",1);
#endif
    while(__atomic_load_n(&completed,__ATOMIC_SEQ_CST)<2)call3(186,0,0,0);
#elif defined(CASE_syscall_abi)
    show("set_tid_address(218)=",call3(218,(word)&done,0,0));
    show("set_robust_list(273)=",call3(273,0,24,0));
    show("rseq(334)=",call3(334,0,0,0));
#elif defined(CASE_untraced_clone)
    long child=call3(56,0x00800000|17,0,0);
    if(child==0) {
        show("untraced child getpid=",call3(39,0,0,0));
        return 0;
    }
    show("parent clone result=",child);
    show("parent wait result=",call3(61,child,0,0));
#elif defined(CASE_shared_pipe)
    show("thread result=",make_thread(0x10f00,child_stack+sizeof child_stack,0,0));
    while(!__atomic_load_n(&done,__ATOMIC_ACQUIRE)) call3(186,0,0,0);
    char byte=0;
    show("parent shared pipe read=",call3(0,descriptors[0],(word)&byte,1));
#elif defined(CASE_wait_status)
    long child=call3(57,0,0,0);
    if(!child)return 42;
    static char status[16] __attribute__((aligned(8)));
    for(int i=0;i<16;i++)status[i]=(char)0xaa;
    show("unaligned wait result=",call3(61,child,(word)(status+7),0));
    show("status byte 7=",(unsigned char)status[7]);
    show("status byte 8=",(unsigned char)status[8]);
#elif defined(CASE_thread_reuse)
    // More total threads than slots, with only one worker alive at a time.
    for(int i=0;i<32;i++) {
        __atomic_store_n(&alive,1,__ATOMIC_RELEASE);
        show("sequential thread=",make_thread(0x210f00,child_stack+sizeof child_stack,0,(word)&alive));
        while(__atomic_load_n(&alive,__ATOMIC_ACQUIRE))call3(186,0,0,0);
    }
#elif defined(CASE_inherited_fds)
    char buffer[32]={0};
    show("inherited read=",call3(0,9,(word)buffer,20));
    long fd=call3(32,9,0,0);
    show("dup inherited fd=",fd);
    show("read alias=",call3(0,fd,(word)buffer,20));
    call3(1,1,(word)buffer,20);
    // close always runs on the host, so this reports whether the launcher's
    // descriptor 9 reached the tracee at all; the reads above are also
    // denied by the supervisor's gates and cannot tell.
    show("inherited close=",call3(3,9,0,0));
#elif defined(CASE_thread_exec)
    show("exec thread=",make_thread(0x10f00,child_stack+sizeof child_stack,0,0));
    for(;;)call3(186,0,0,0);
#elif defined(CASE_exec_failure)
    show("exec thread=",make_thread(0x10f00,child_stack+sizeof child_stack,0,0));
    while(!__atomic_load_n(&done,__ATOMIC_ACQUIRE))call3(186,0,0,0);
    show("parent resumed=",call3(186,0,0,0));
#elif defined(CASE_copied_files)
    call3(22,(word)descriptors,0,0);
    show("copied-files thread=",make_thread(0x10b00,child_stack+sizeof child_stack,0,0));
    while(!__atomic_load_n(&done,__ATOMIC_ACQUIRE))call3(186,0,0,0);
    show("parent pipe write=",call3(1,descriptors[1],(word)"x",1));
#elif defined(CASE_shared_exec)
    call3(293,(word)descriptors,0x80000,0);
    long child=call3(56,0x411,0,0);
    if(!child) {
        char *argv[]={exec_path,0};
        return call3(59,(word)exec_path,(word)argv,0);
    }
    show("shared exec wait=",call3(61,child,0,0));
    show("parent pipe write=",call3(1,descriptors[1],(word)"x",1));
#elif defined(CASE_clone_tid)
    long child=make_thread(0x1300011,child_stack+sizeof child_stack,(word)&parent_tid,(word)&child_tid);
    show("parent tid cell=",parent_tid);
    show("clone return=",child);
    show("clone wait=",call3(61,child,0,0));
#elif defined(CASE_child_signal)
    long child=call3(57,0,0,0);
    if(!child) {
        call3(62,call3(39,0,0,0),15,0);
        return 99;
    }
    int status=0;
    show("signal wait=",call3(61,child,(word)&status,0));
    show("signal status=",status);
#elif defined(CASE_orphan_reparent) || defined(CASE_orphan_zombie)
    // The middle process exits without reaping its child; init must then be
    // that child's parent and able to wait for it.
    if(call3(22,(word)descriptors,0,0))return 10;
    long middle=call3(57,0,0,0);
    if(!middle) {
        long child=call3(57,0,0,0);
        if(!child) {
#if defined(CASE_orphan_reparent)
            char byte;
            call3(0,descriptors[0],(word)&byte,1);
            show("orphan getppid=",call3(110,0,0,0));
#else
            call3(1,descriptors[1],(word)"x",1);
#endif
            return 7;
        }
        if(child<0)return 11;
#if defined(CASE_orphan_zombie)
        // Let the child exit first, so it is already dead when adopted.
        char byte;
        call3(0,descriptors[0],(word)&byte,1);
        for(int i=0;i<100;i++)call3(186,0,0,0);
#endif
        return 0;
    }
    int status=0;
    show("orphan middle wait=",call3(61,middle,(word)&status,0));
#if defined(CASE_orphan_reparent)
    call3(1,descriptors[1],(word)"x",1);
#endif
    status=0;
    show("orphan wait=",call3(61,-1,(word)&status,0));
    show("orphan status=",status);
#elif defined(CASE_sigpipe)
    long child=call3(57,0,0,0);
    if(!child) {
        // Writing to a pipe without readers raises SIGPIPE in the writer.
        if(call3(22,(word)descriptors,0,0))return 10;
        call3(3,descriptors[0],0,0);
        show("sigpipe write=",call3(1,descriptors[1],(word)"x",1));
        return 99;
    }
    int status=0;
    show("sigpipe wait=",call3(61,child,(word)&status,0));
    show("sigpipe status=",status);
#elif defined(CASE_segv)
    long child=call3(57,0,0,0);
    if(!child) {
        static int *volatile null_pointer;
        *null_pointer=1;
        return 99;
    }
    int status=0;
    show("segv wait=",call3(61,child,(word)&status,0));
    show("segv status=",status);
#elif defined(CASE_thread_segv)
    long child=call3(57,0,0,0);
    if(!child) {
        if(make_thread(0x10f00,child_stack+sizeof child_stack,0,0)<0)return 10;
        for(;;)call3(39,0,0,0);
    }
    int status=0;
    show("thread segv wait=",call3(61,child,(word)&status,0));
    show("thread segv status=",status);
#elif defined(CASE_thread_term)
    if(call3(22,(word)descriptors,0,0))return 10;
    long child=call3(57,0,0,0);
    if(!child) {
        if(make_thread(0x10f00,child_stack+sizeof child_stack,0,0)<0)return 11;
        while(!__atomic_load_n(&done,__ATOMIC_ACQUIRE))call3(39,0,0,0);
        call3(1,descriptors[1],(word)"x",1);
        for(;;)call3(39,0,0,0);
    }
    // Signal the child only once both of its threads are making syscalls.
    char byte=0;
    if(call3(0,descriptors[0],(word)&byte,1)!=1)return 12;
    show("thread term kill=",call3(62,child,15,0));
    int status=0;
    show("thread term wait=",call3(61,child,(word)&status,0));
    show("thread term status=",status);
#elif defined(CASE_parked_kill)
    long middle=call3(57,0,0,0);
    if(!middle) {
        if(call3(22,(word)descriptors,0,0))return 20;
        long grandchild=call3(57,0,0,0);
        if(!grandchild) {
            char byte;
            call3(0,descriptors[0],(word)&byte,1);
            return 0;
        }
        if(make_thread(0x10f00,child_stack+sizeof child_stack,0,0)<0)return 21;
        // The main thread parks in wait4 while its thread faults.
        int inner=0;
        call3(61,grandchild,(word)&inner,0);
        return 22;
    }
    int status=0;
    show("parked kill wait=",call3(61,middle,(word)&status,0));
    show("parked kill status=",status);
#elif defined(CASE_parked_term)
    static int ready[2];
    if(call3(22,(word)ready,0,0))return 10;
    long middle=call3(57,0,0,0);
    if(!middle) {
        if(call3(22,(word)descriptors,0,0))return 20;
        long grandchild=call3(57,0,0,0);
        if(!grandchild) {
            // Reads EOF once the middle task dies with the last write end.
            char byte;
            call3(3,descriptors[1],0,0);
            call3(0,descriptors[0],(word)&byte,1);
            return 0;
        }
        call3(3,descriptors[0],0,0);
        call3(1,ready[1],(word)"x",1);
        // Parks in wait4 until the parent's SIGTERM ends it.
        call3(61,grandchild,0,0);
        return 21;
    }
    char byte=0;
    if(call3(0,ready[0],(word)&byte,1)!=1)return 11;
    // Give the middle task time to park after its ready byte.
    for(int i=0;i<100;i++)call3(186,0,0,0);
    show("parked term kill=",call3(62,middle,15,0));
    int status=0;
    show("parked term wait=",call3(61,middle,(word)&status,0));
    show("parked term status=",status);
#elif defined(CASE_ro_status) || defined(CASE_bad_status) || defined(CASE_bad_rusage) || \
    defined(CASE_straddle_status)
    // wait4 stores like the kernel's own copy to user memory: a status or
    // rusage pointer the caller cannot write fails with EFAULT after the
    // child is reaped, the status before the rusage, and a failed status
    // store leaves the rusage and the status word untouched. The first round
    // finds the child exited (WNOHANG), the second parks while the child
    // spins in syscalls.
    static const int ro_cell __attribute__((section(".rodata"), aligned(8)));
    static int usage[36];
    const word unmapped=0x1000;
    // The last two bytes before the unmapped page after the bss: a status
    // word there crosses into memory the caller cannot write.
    extern char _end[];
    word page_end=((word)_end+4095)&~(word)4095;
    if(page_end-(word)_end<2)return 30;
    volatile unsigned char *edge=(volatile unsigned char *)(page_end-2);
    for(int round=0;round<2;round++) {
        for(int i=0;i<36;i++)usage[i]=0x5a5a5a5a;
        edge[0]=0x77;
        edge[1]=0x77;
        long child=call3(57,0,0,0);
        if(!child) {
            for(int i=0;round && i<100;i++)call3(186,0,0,0);
            return 42;
        }
        int status=0x12345678;
#if defined(CASE_ro_status)
        word status_ptr=(word)&ro_cell, rusage_ptr=(word)usage;
#elif defined(CASE_bad_status)
        word status_ptr=unmapped, rusage_ptr=(word)usage;
#elif defined(CASE_straddle_status)
        word status_ptr=page_end-2, rusage_ptr=(word)usage;
#else
        word status_ptr=(word)&status, rusage_ptr=unmapped;
#endif
        long result;
        if(round) {
            result=call4(61,child,status_ptr,0,rusage_ptr);
        } else {
            while(!(result=call4(61,child,status_ptr,1,rusage_ptr)))call3(186,0,0,0);
        }
        show(round?"blocking wait=":"nohang wait=",result);
        show(round?"blocking ro cell=":"nohang ro cell=",*(volatile const int *)&ro_cell);
        show(round?"blocking edge=":"nohang edge=",edge[0]|edge[1]<<8);
        show(round?"blocking status=":"nohang status=",status);
        show(round?"blocking rusage=":"nohang rusage=",usage[0]);
        show(round?"blocking rewait=":"nohang rewait=",call3(61,child,0,0));
    }
#elif defined(CASE_kill_badsig)
    long pid=call3(39,0,0,0);
    show("kill signal 65=",call3(62,pid,65,0));
    show("kill signal -1=",call3(62,pid,(word)-1,0));
    show("kill missing pid signal 65=",call3(62,77,65,0));
#elif defined(CASE_nonblocking_wait)
    call3(22,(word)descriptors,0,0);
    long child=call3(57,0,0,0);
    if(!child) {
        char byte;
        call3(0,descriptors[0],(word)&byte,1);
        return 0;
    }
    show("nonblocking wait=",call3(61,0xffffffff,0,1));
    call3(1,descriptors[1],(word)"x",1);
    show("blocking wait=",call3(61,0xffffffff,0,0));
#elif defined(CASE_vfork)
    long child=call3(58,0,0,0);
    if(!child) {
        call3(60,42,0,0);
        __builtin_unreachable();
    }
    int status=0;
    show("vfork return=",child);
    show("vfork wait=",call3(61,child,(word)&status,0));
    show("vfork status=",status);
#elif defined(CASE_unused_parent_tid)
    show("no-parent-settid thread=",make_thread(0x10f00,child_stack+sizeof child_stack,1,0));
    while(!__atomic_load_n(&done,__ATOMIC_ACQUIRE))call3(186,0,0,0);
#elif defined(CASE_denied_host_memory)
    // A null iovec makes this a harmless host permission/error probe.
    register long r10 __asm__("r10")=0;
    register long r8 __asm__("r8")=0;
    register long r9 __asm__("r9")=0;
    long result;
    __asm__ volatile("syscall" : "=a"(result) : "a"(311),"D"(0),"S"(0),"d"(0),"r"(r10),"r"(r8),"r"(r9) : "rcx","r11","memory");
    show("process_vm_writev(311)=",result);
#elif defined(CASE_alias_writes)
    // Every alias keeps the write rule of the stdio descriptor it names:
    // stdin stays unwritable, stdout keeps the console bound.
    static char large[4097];
    show("plain stdin write=",call3(1,0,(word)"via-0\n",6));
    show("plain stdout write 4097=",call3(1,1,(word)large,sizeof large));
    long alias=call3(32,0,0,0);
    show("dup stdin write=",call3(1,alias,(word)"via-dup\n",8));
    long chained=call3(32,alias,0,0);
    show("dup alias write=",call3(1,chained,(word)"via-dup-dup\n",12));
    call3(33,0,7,0);
    show("dup2 stdin write=",call3(1,7,(word)"via-dup2\n",9));
    call3(292,0,8,0x80000);
    show("dup3 stdin write=",call3(1,8,(word)"via-dup3\n",9));
    long fcntl_alias=call3(72,0,0,10);
    show("fcntl dupfd stdin write=",call3(1,fcntl_alias,(word)"via-dupfd\n",10));
    long cloexec_alias=call3(72,0,1030,10);
    show("fcntl dupfd-cloexec stdin write=",call3(1,cloexec_alias,(word)"via-dupfd-cloexec\n",18));
    long saved=call3(32,1,0,0);
    call3(33,0,1,0);
    long over=call3(1,1,(word)"via-1\n",6);
    call3(33,saved,1,0);
    show("stdin over stdout write=",over);
    show("stdout alias write=",call3(1,saved,(word)"stdout alias ok\n",16));
    show("stdout alias write 4097=",call3(1,saved,(word)large,sizeof large));
    show("restored stdout write 4097=",call3(1,1,(word)large,sizeof large));
    long stderr_alias=call3(32,2,0,0);
    show("stderr alias write=",call3(1,stderr_alias,(word)"stderr alias ok\n",16));
    show("stderr alias write 4097=",call3(1,stderr_alias,(word)large,sizeof large));
#endif
    return 0;
}
__asm__(".global _start\n_start:\nmov %rsp,%rdi\nand $-16,%rsp\ncall probe_main\nmov %rax,%rdi\nmov $60,%rax\nsyscall\n");
