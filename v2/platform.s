// Fixed game platform functions. Preserve guest state as the reference hooks do.
Lsqrt:
 movups %xmm0,-16(%rsp)
 sqrtsd %xmm1,%xmm0
 movsd %xmm0,(%r8)
 movups -16(%rsp),%xmm0
 ret
Ltimer: movq $0,(%rdx)
 ret
Lportrait: movb $0,(%rdx)
 ret
Lalloc:
 pushfq
 cmpq $2,%rdx
 je Lalloc_done
 ja Lbad
 pushq %rax
 pushq %rcx
 pushq %rdi
 pushq %rsi
 pushq %r10
 movq Lcursor(%rip),%r10
 movq %r8,%rcx
 addq $15,%rcx
 jc Lbad
 andq $-16,%rcx
 jnz Lsize
 movl $16,%ecx
Lsize:
 movq %r10,%rax
 addq %rcx,%rax
 jc Lbad
 cmpq $0x40000000,%rax
 ja Lbad
 movq %rax,Lcursor(%rip)
 movq %r10,%rdi
 xorl %eax,%eax
 rep stosb
 cmpq $1,%rdx
 jne Lalloc_out
 movq 0x58(%rsp),%rsi
 testq %rsi,%rsi
 jz Lalloc_out
 movq %r8,%rcx
 cmpq %r9,%rcx
 cmovaq %r9,%rcx
 movq %r10,%rdi
 rep movsb
Lalloc_out:
 movq 0x68(%rsp),%rax
 movq %r10,(%rax)
 popq %r10
 popq %rsi
 popq %rdi
 popq %rcx
 popq %rax
Lalloc_done:
 popfq
 ret
Lbad: ud2
Lmemset:
 pushq %rcx
 pushq %rdi
 movq %rcx,%rdi
 movq %r8,%rcx
 movq %rdx,%rax
 rep stosb
 popq %rdi
 popq %rcx
 movq %rcx,%rax
 ret
Lmemcpy:
Lmemmove:
 pushfq
 pushq %rcx
 pushq %rsi
 pushq %rdi
 movq %rcx,%rdi
 movq %rdx,%rsi
 movq %r8,%rcx
 cmpq %rsi,%rdi
 jbe Lforward
 leaq -1(%rdi,%rcx),%rdi
 leaq -1(%rsi,%rcx),%rsi
 std
Lforward: rep movsb
 popq %rdi
 popq %rsi
 popq %rcx
 popfq
 movq %rcx,%rax
 ret
Lmemcmp:
 pushfq
 pushq %rcx
 pushq %rsi
 pushq %rdi
 movq %rcx,%rsi
 movq %rdx,%rdi
 movq %r8,%rcx
 xorl %eax,%eax
 repe cmpsb
 je Lcompdone
 movl $1,%eax
 ja Lcompdone
 movq $-1,%rax
Lcompdone:
 popq %rdi
 popq %rsi
 popq %rcx
 popfq
 ret
