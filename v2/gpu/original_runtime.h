#pragma once
#include "original_image.h"
#include "d3dmetal_host.h"
#include <unordered_set>
#include "deferred_pipelines.h"
#include <signal.h>
#include <sys/ucontext.h>
#include <pthread.h>
// Minimal platform services for isolated original render functions.
// No process entry, TLS callbacks, game loop, window, Steam or Wine runtime.
struct OriginalRuntime {
 OriginalImage image;
 inline static std::unordered_set<void*> allocations;
 inline static uint64_t completedShaderJobs=0;
 inline static std::vector<void*> renderJobs,publishedRenderJobs,commandJobs,publishedCommandJobs;
 static void __attribute__((ms_abi)) waitShaderJobs(void*){auto pending=*(uint64_t*)0x1410d5678;fprintf(stderr,"Synchronous shader jobs: pending=%llu completed=%llu\n",(unsigned long long)pending,(unsigned long long)completedShaderJobs);if(pending!=completedShaderJobs)abort();completedShaderJobs=0;*(uint64_t*)0x1410d5678=0;}
 static void __attribute__((ms_abi)) collectShaderJobs(void*,void*queue,uint64_t*out){if(queue==(void*)0x140b5dee8ull){publishedCommandJobs=std::move(commandJobs);commandJobs.clear();out[0]=publishedCommandJobs.size();out[1]=(uintptr_t)publishedCommandJobs.data();return;}if(queue==(void*)0x140b5de88ull){publishedRenderJobs=std::move(renderJobs);renderJobs.clear();out[0]=publishedRenderJobs.size();out[1]=(uintptr_t)publishedRenderJobs.data();return;}if(queue!=(void*)0x140b5df48ull){fprintf(stderr,"Unbridged job queue %p caller=%p\n",queue,__builtin_return_address(0));abort();}*out=completedShaderJobs;completedShaderJobs=0;}
 static void* __attribute__((ms_abi)) fill(void*p,int c,size_t n){return memset(p,c,n);}
 static int __attribute__((ms_abi)) compare(const void*p,const void*q,size_t n){return memcmp(p,q,n);}
 static void* __attribute__((ms_abi)) copy(void*p,const void*q,size_t n){return memcpy(p,q,n);}
 static void __attribute__((ms_abi)) allocate(void*,uint64_t op,size_t n,size_t oldSize,void *old,uint64_t,void **out){
  if(n>512*1024*1024||op>2){fprintf(stderr,"Bad allocation n=%zu op=%llu caller=%p\n",n,(unsigned long long)op,__builtin_return_address(0));abort();}
  if(old&&allocations.erase(old)!=1){fprintf(stderr,"Unknown original allocation %p op=%llu n=%zu caller=%p\n",old,(unsigned long long)op,n,__builtin_return_address(0));abort();}
  void *p=nullptr;
  if(op==2){free(old);}else{p=realloc(old,n?n:1);if(!p)abort();if(n>oldSize)memset((char*)p+oldSize,0,n-oldSize);allocations.insert(p);}
  *out=p;
 }
 static int __attribute__((ms_abi)) widen(unsigned code,unsigned,const char*src,int n,uint16_t*dst,int capacity){
  if(code!=65001||n<0)abort();if(dst){if(capacity<n)return 0;for(int i=0;i<n;i++){if((unsigned char)src[i]>=128)abort();dst[i]=(unsigned char)src[i];}}return n;
 }
 static void __attribute__((ms_abi)) shaderJob(void *ctx,void *queue,void **shader,void*){
  if(queue==(void*)0x140b5de88ull||queue==(void*)0x140b5dee8ull){Bytes worker(4096);memcpy(worker.data(),ctx,0x300);*(uint64_t*)(worker.data()+8)=1;uint64_t done=0;using Work=void (__attribute__((ms_abi)) *)(void*,uint64_t,uint64_t,void*,uint64_t*);((Work)(queue==(void*)0x140b5de88ull?0x140226d90ull:0x140229330ull))(worker.data(),0,0,shader,&done);if(!done)abort();(queue==(void*)0x140b5de88ull?renderJobs:commandJobs).push_back(shader);return;}
  if(queue!=(void*)0x140b5df48ull){fprintf(stderr,"Unbridged job queue %p caller=%p\n",queue,__builtin_return_address(0));abort();}
  if(DeferredPipelines::defer(ctx,shader)){completedShaderJobs++;return;}
  using F=void (__attribute__((ms_abi)) *)(void*,uint64_t,uint64_t,void**,uint64_t*);uint64_t done=0;((F)0x140229540ull)(ctx,0,0,shader,&done);if(!done)abort();completedShaderJobs++;
 }
 static void* __attribute__((ms_abi)) fileRequest(const uint16_t*path,uint32_t,uint32_t,void*,uint32_t,uint32_t,void*){std::string name;while(*path)name.push_back(char(*path++));fprintf(stderr,"Unbridged original file request: %s\n",name.c_str());auto stack=(uintptr_t*)__builtin_frame_address(0);for(int i=0;i<512;i++)if(stack[i]>=0x140001000ull&&stack[i]<0x140662000ull)fprintf(stderr,"  caller candidate: %llx\n",(unsigned long long)stack[i]);_Exit(7);}
 static void* __attribute__((ms_abi)) crtMalloc(size_t n){return malloc(n);}
 static void __attribute__((ms_abi)) crtFree(void*p){free(p);}
 static void* __attribute__((ms_abi)) crtCalloc(size_t n,size_t s){return calloc(n,s);}
 static void* __attribute__((ms_abi)) crtRealloc(void*p,size_t n){return realloc(p,n);}
 static void fault(int signal,siginfo_t *info,void *context){auto &r=((ucontext_t*)context)->uc_mcontext->__ss;fprintf(stderr,"Native bridge fault %d address=%p rip=%llx rcx=%llx rdx=%llx r8=%llx r9=%llx rax=%llx rsp=%llx\n",signal,info->si_addr,r.__rip,r.__rcx,r.__rdx,r.__r8,r.__r9,r.__rax,r.__rsp);if(r.__rip==0x1402af3bc)fprintf(stderr,"Missing mesh entity uid=%u type=%llx flags=%llx\n",*(unsigned*)(r.__rsi+0x100),*(unsigned long long*)(r.__rsi+0x108),*(unsigned long long*)(r.__rsi+0x28));fprintf(stderr,"return=%llx\n",*(unsigned long long*)r.__rsp);auto top=(uintptr_t)pthread_get_stackaddr_np(pthread_self());auto end=std::min(top,(uintptr_t)r.__rsp+16384);for(auto p=(uintptr_t*)r.__rsp;(uintptr_t)p<end;p++)if(*p>=0x140001000ull&&*p<0x140662000ull)fprintf(stderr,"  stack code candidate %llx\n",(unsigned long long)*p);_Exit(128+signal);}
 OriginalRuntime(const char*exe,Host &host):image(exe){
  struct sigaction action={};action.sa_sigaction=fault;action.sa_flags=SA_SIGINFO;sigaction(SIGSEGV,&action,nullptr);sigaction(SIGBUS,&action,nullptr);sigaction(SIGILL,&action,nullptr);sigaction(SIGFPE,&action,nullptr);sigaction(SIGABRT,&action,nullptr);
  *(void**)0x1407c3560=(void*)crtCalloc;*(void**)0x1407c3568=(void*)crtFree;*(void**)0x1407c3570=(void*)crtMalloc;*(void**)0x1407c3578=(void*)crtRealloc;
  uint8_t ret=0xc3;image.patch(0x1404476e0,&ret,1); // Windows TEB stack probe replaced for committed host stack.
  image.redirect(0x1406615c0,(void*)compare);image.redirect(0x1406615f0,(void*)fill);image.redirect(0x1406615d0,(void*)copy);image.redirect(0x140001000,(void*)allocate);
  *(void**)0x1407c3068=(void*)widen;*(void**)0x1407c2e80=(void*)fileRequest;
  *(void**)0x14071e878=(void*)allocate;*(void**)0x14071e908=(void*)allocate;*(void**)0x140807270=(void*)allocate;
  *(uint64_t*)0x140b5dec8=1;*(ID3D12Device**)0x140884f58=host.d;image.redirect(0x14022b0c0,(void*)collectShaderJobs);image.redirect(0x1400c82d0,(void*)waitShaderJobs);
 }
 ~OriginalRuntime(){for(auto p:allocations)free(p);allocations.clear();}
 static void __attribute__((ms_abi)) log(void*,uint64_t*str,uint64_t,void*){fprintf(stderr,"Original: %.*s\n",int(str[0]),(const char*)str[1]);}
 static void __attribute__((ms_abi)) assertion(void*,void*location,uint64_t*str,void*){fprintf(stderr,"Original assertion at %p caller=%p: %.*s\n",location,__builtin_return_address(0),int(str[0]),(const char*)str[1]);if(location){auto p=(uint64_t*)location;fprintf(stderr,"location words: %llx %llx %llx %llx\n",(unsigned long long)p[0],(unsigned long long)p[1],(unsigned long long)p[2],(unsigned long long)p[3]);}abort();}
 void context(Bytes &ctx){ctx.resize(4096);using Init=void (__attribute__((ms_abi)) *)(void*);image.function<Init>(0x1400717e0)(ctx.data());*(uint32_t*)(ctx.data()+8)=0;*(void**)(ctx.data()+0x10)=(void*)allocate;*(void**)(ctx.data()+0x20)=(void*)log;*(void**)(ctx.data()+0x50)=(void*)assertion;*(void**)(ctx.data()+0x80)=(void*)0x140884f58ull;}
};
