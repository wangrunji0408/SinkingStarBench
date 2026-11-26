#pragma once
#include "original_runtime.h"
inline void initializeOriginalHeaps(Host&host,OriginalRuntime&r){
  using HeapInit=void (__attribute__((ms_abi)) *)(void*,void*,uint32_t,uint32_t,uint32_t,uint32_t);
  auto heapInit=r.image.function<HeapInit>(0x14015c880);
  heapInit(host.d,(void*)0x1410d6d38,3,0,65536,65536);
  heapInit(host.d,(void*)0x1410d6c90,2,0,65536,65536);
  heapInit(host.d,(void*)0x1410d6de0,0,0,300000,300000);
  heapInit(host.d,(void*)0x1410d6e88,0,1,200000,400000);
}
