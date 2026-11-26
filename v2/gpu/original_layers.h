#pragma once
// Isolated original 0x1404380cf..0x140438d64 layer-table setup.
extern "C" void __attribute__((naked,ms_abi)) originalLayerEnd(){
 __asm__ volatile(
 "movaps 0x2f00(%rsp), %xmm6\n"
 "movaps 0x2f10(%rsp), %xmm7\n"
 "movaps 0x2f20(%rsp), %xmm8\n"
 "movaps 0x2f30(%rsp), %xmm9\n"
 "movaps 0x2f40(%rsp), %xmm10\n"
 "movaps 0x2f50(%rsp), %xmm11\n"
 "movaps 0x2f60(%rsp), %xmm12\n"
 "movaps 0x2f70(%rsp), %xmm13\n"
 "movaps 0x2f80(%rsp), %xmm14\n"
 "movaps 0x2f90(%rsp), %xmm15\n"
 "addq $0x3008, %rsp\n popq %r15\n popq %r14\n popq %r13\n popq %r12\n popq %rsi\n popq %rdi\n popq %rbx\n popq %rbp\n retq\n");
}
extern "C" void __attribute__((naked,ms_abi)) originalSetupSection(void*,void*,uintptr_t){
 __asm__ volatile("pushq %rbp\n pushq %rbx\n pushq %rdi\n pushq %rsi\n pushq %r12\n pushq %r13\n pushq %r14\n pushq %r15\n subq $0x3008, %rsp\n"
 "movaps %xmm6, 0x2f00(%rsp)\n"
 "movaps %xmm7, 0x2f10(%rsp)\n"
 "movaps %xmm8, 0x2f20(%rsp)\n"
 "movaps %xmm9, 0x2f30(%rsp)\n"
 "movaps %xmm10, 0x2f40(%rsp)\n"
 "movaps %xmm11, 0x2f50(%rsp)\n"
 "movaps %xmm12, 0x2f60(%rsp)\n"
 "movaps %xmm13, 0x2f70(%rsp)\n"
 "movaps %xmm14, 0x2f80(%rsp)\n"
 "movaps %xmm15, 0x2f90(%rsp)\n"
 "xorps %xmm6, %xmm6\n"
 "movq %rcx, %r13\n movq %rdx, %r14\n movq %rcx, 0x58(%rsp)\n movq %rdx, 0xa0(%rsp)\n movq %rdx, 0xa8(%rsp)\n jmpq *%r8\n");
}
