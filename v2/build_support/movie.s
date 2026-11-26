// Observe original particle requests; ordinary replays take the fast path.
Lparticle_observer:
 pushfq
 cmpb $0,Lobserve(%rip)
 je Lparticle_continue
 pushq %rax
 pushq %rbx
 pushq %rcx
 pushq %rdx
 pushq %rsi
 pushq %rdi
 pushq %r8
 pushq %r9
 pushq %r10
 pushq %r11
 leaq 88(%rsp), %rdi
 movq %rdx,%rsi
 movq %r8,%rdx
 movq %r9,%rcx
 subq $256,%rsp
 .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
 movups %xmm\n,16*\n(%rsp)
 .endr
 movq %rsp,%rbx
 movq Lhostsp(%rip),%rsp
 andq $-16,%rsp
 callq *Lobserve_fn(%rip)
 movq %rbx,%rsp
 .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
 movups 16*\n(%rsp),%xmm\n
 .endr
 addq $256,%rsp
 popq %r11
 popq %r10
 popq %r9
 popq %r8
 popq %rdi
 popq %rsi
 popq %rdx
 popq %rcx
 popq %rbx
 popq %rax
Lparticle_continue:
 popfq
 pushq %r15
 pushq %r14
 pushq %r13
 jmp Lparticle_resume
Lobserve_fn: .quad _star_particle_event
.section __BRIDGE,__data
Lobserve: .byte 0
.data
.globl _observe_flag
_observe_flag: .quad Lobserve
.section __GLUE,__text,regular,pure_instructions
