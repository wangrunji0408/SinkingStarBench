#pragma once
#include "original_world.h"
struct OriginalGPUBuffers {
 // Original 0x140177000 packs the ocean index into 16 bits. The game reserves
 // only 128 CPU/GPU records; a complete static overworld exceeds that reserve.
 static constexpr uint32_t oceanCapacity=65536;
 static void __attribute__((ms_abi)) oceanRecord(void**out,uint32_t*index){auto count=(uint32_t*)0x140b5a300;if(*count>=oceanCapacity)throw std::runtime_error("Ocean record capacity exceeded");*index=(*count)++;*out=*(uint8_t**)0x140b5a2f8+size_t(*index)*16;}

 std::vector<IUnknown*> transientUploads;
 void releaseUploads(){for(auto resource:transientUploads){auto it=std::find(host.objects.begin(),host.objects.end(),resource);if(it!=host.objects.end())host.objects.erase(it);resource->Release();}transientUploads.clear();}
 std::vector<Bytes> cpuMemory;Host&host;CommandBatch initial;ID3D12GraphicsCommandList*list;inline static OriginalGPUBuffers*active;std::unordered_set<void*> uploads;
 OriginalGPUBuffers(Host&h,OriginalRuntime&r,ID3D12GraphicsCommandList*l):host(h),initial(h),list(l){active=this;r.image.redirect(0x14016c880,(void*)oceanRecord);r.image.redirect(0x14011a5f0,(void*)update);r.image.redirect(0x1400cd600,(void*)writeView);r.image.redirect(0x14016dcc0,(void*)transition);
#include "original_global_buffers.inc"
 initial.wait();r.image.redirect(0x14011a5f0,(void*)update);r.image.redirect(0x1400cd600,(void*)writeView);r.image.redirect(0x14016dcc0,(void*)transition);
 }
 D3D12_CPU_DESCRIPTOR_HANDLE descriptor(uint8_t*p,bool visible=false){using Alloc=void (__attribute__((ms_abi)) *)(void*,void*,uint32_t,void*);((Alloc)0x1400e6eb0ull)((void*)0x140807260,(void*)(visible?0x1410d6e88ull:0x1410d6de0ull),1,p);return {*(uint64_t*)p};}
 void create(uint8_t*b,uint32_t size,uint32_t stride,uint16_t usage){if((uintptr_t)b<0x140b00000ull)return;using Create=void (__attribute__((ms_abi)) *)(void*,void*,uint32_t,uint32_t,void*,uint16_t,void*,uint8_t,uint8_t);uint64_t name[2]={0,0};((Create)0x140165620ull)((void*)0x140807260,b,size,stride,nullptr,usage,name,0,0);
 }
 static void __attribute__((ms_abi)) writeView(void*ctx,uint8_t*v,void*source,uint8_t){auto b=*(uint8_t**)v;update(ctx,b,*(uint64_t*)(v+8),*(uint32_t*)(v+0x14),source,0,0);}
 static void __attribute__((ms_abi)) transition(void*,uint8_t*data,uint32_t state,uint32_t sub,uint8_t*out){if(OriginalTextureCatalog::active)OriginalTextureCatalog::active->ensure(data);auto resource=*(ID3D12Resource**)data;if(!resource){fprintf(stderr,"Uninitialized GPU resource %p\n",data);_Exit(6);}if(!active->uploads.count(data)&&!materialUploadData.count(data)&&!MeshCatalog::active->bufferData.count(data)){auto old=*(uint32_t*)(data+16);if(old!=state){D3D12_RESOURCE_BARRIER b={};b.Type=D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;b.Transition={resource,sub,D3D12_RESOURCE_STATES(old),D3D12_RESOURCE_STATES(state)};active->list->ResourceBarrier(1,&b);*(uint32_t*)(data+16)=state;}}if(out)*out=0;}
 static void __attribute__((ms_abi)) update(void*ctx,uint8_t*b,uint32_t offset,uint32_t size,void*source,uint8_t,uint8_t){if(getenv("SINKING_DUMP_BUFFERS")&&source&&size<8192){static int seq=0;std::ofstream f(std::string(getenv("SINKING_DUMP_BUFFERS"))+"/"+std::to_string(seq++)+"-"+std::to_string(size)+".bin",std::ios::binary);f.write((char*)source,size);}
 auto data=*(uint8_t**)b;auto resource=*(ID3D12Resource**)data;if(!source||offset+uint64_t(size)>*(uint32_t*)(b+0x40)){fprintf(stderr,"Bad original buffer write b=%p offset=%u size=%u capacity=%u source=%p caller=%p\n",b,offset,size,*(uint32_t*)(b+0x40),source,__builtin_return_address(0));abort();}if(active->uploads.count(data)||materialUploadData.count(data)||MeshCatalog::active->bufferData.count(data)){void*p;D3D12_RANGE none={0,0};check(resource->Map(0,&none,&p),"Map dynamic original buffer");memcpy((uint8_t*)p+offset,source,size);D3D12_RANGE written={offset,size_t(offset)+size};resource->Unmap(0,&written);}else{Bytes bytes((uint8_t*)source,(uint8_t*)source+size);auto upload=active->host.upload(bytes);active->transientUploads.push_back(upload);auto old=*(uint32_t*)(data+16);uint8_t done;transition(ctx,data,D3D12_RESOURCE_STATE_COPY_DEST,~0u,&done);active->list->CopyBufferRegion(resource,offset,upload,0,size);transition(ctx,data,old,~0u,&done);}}
};
