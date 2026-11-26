#pragma once
#include "original_runtime.h"
inline static Host *materialGPU;
inline static std::unordered_set<void*> materialUploadData;
static void __attribute__((ms_abi)) constantBuffer(void*,uint8_t *view,uint32_t size,void *data){
 Bytes bytes(size);if(data)memcpy(bytes.data(),data,size);auto resource=materialGPU->upload(bytes);auto heap=materialGPU->heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,1,false);auto handle=cpuHandle(heap);D3D12_CONSTANT_BUFFER_VIEW_DESC cb={resource->GetGPUVirtualAddress(),UINT((size+255)&~255u)};materialGPU->d->CreateConstantBufferView(&cb,handle);
 void *buffer=nullptr;OriginalRuntime::allocate(nullptr,0,272,0,nullptr,0,&buffer);auto b=(uint8_t*)buffer;materialUploadData.insert(b+8);*(void**)b=b+8;*(void**)(b+8)=resource;*(uint32_t*)(b+0x18)=D3D12_RESOURCE_STATE_GENERIC_READ;*(uint32_t*)(b+0x40)=size;*(uint32_t*)(b+0x48)=1;b[0x4c]=1;*(uint64_t*)(b+0xa8)=handle.ptr;
 memset(view,0,88);*(void**)view=b;*(uint32_t*)(view+0x10)=1;*(uint32_t*)(view+0x14)=size;*(uint64_t*)(view+0x18)=handle.ptr;*(uint32_t*)(view+0x28)=1;
}
