#pragma once
#include "original_runtime.h"
#include "d3dmetal_textures.h"
#include <future>
#include <unordered_map>
// Reuse a bounded staging buffer. The fence protects its contents (and the
// allocator) before the next batch overwrites them. Texture descriptors stay
// in catalog order regardless of where an individual upload batch ends.
struct TextureUploadBatch {
 Host& host;
 CommandBatch commands;
 ID3D12Resource* buffer=nullptr;
 size_t capacity=0, used=0;
 explicit TextureUploadBatch(Host& h):host(h),commands(h){}
 void flush(){
  if(used){commands.wait();commands.reset();used=0;}
 }
 void append(OriginalD3DTexture& texture){
  size_t n=texture.payload.size(), offset=(used+511)&~size_t(511);
  if(offset+n>capacity){
   flush();offset=0;
   if(n>capacity){
    capacity=std::max(n,size_t(32)<<20);
    buffer=host.buffer(capacity);
   }
  }
  void* dst;D3D12_RANGE none={0,0};
  check(buffer->Map(0,&none,&dst),"Map texture batch");
  memcpy((uint8_t*)dst+offset,texture.payload.data(),n);
  D3D12_RANGE written={offset,offset+n};buffer->Unmap(0,&written);
  texture.upload(host,commands.list,buffer,offset);
  Bytes().swap(texture.payload);
  used=offset+n;
 }
};
struct OriginalTextureCatalog {
 inline static OriginalTextureCatalog* active=nullptr;
 TextureUploadBatch upload;
 std::unordered_map<void*,size_t> pending;
 // The original material binder requests the resource before recording a draw.
 // Keep descriptor slots stable and fence uploads before submitting that draw.
 void ensure(void* data){
  auto it=pending.find(data);if(it==pending.end())return;
  auto& t=textures[it->second];t.payload=gamePayload(sourceBytes(t.deferred));
  if(t.payload.size()<t.minimumBytes)throw std::runtime_error("Truncated texture: "+t.name);
  upload.append(t);pending.erase(it);
 }
 std::vector<OriginalD3DTexture> textures;std::vector<Bytes> records;Bytes table;
 static uint16_t sourceFormat(DXGI_FORMAT f){switch(f){case 61:return 1;case 49:return 2;case 28:case 29:return 4;case 87:case 91:return 5;case 10:return 18;case 71:case 72:return 256;case 77:case 78:return 257;case 80:return 258;case 83:return 259;case 95:return 260;case 98:case 99:return 261;default:throw std::runtime_error("Unknown source texture format");}}
 OriginalTextureCatalog(Host &host,const char *path):upload(host){active=this;BundleReader input(path,true);if(input.word()!=0x31545844)throw std::runtime_error("Invalid original texture bundle");auto count=input.word();textures.reserve(count);records.reserve(count);
  uint64_t gpuDesc[4],cpuDesc[4];using Alloc=void (__attribute__((ms_abi)) *)(void*,void*,uint32_t,uint64_t*);auto alloc=(Alloc)0x1400e6eb0ull;alloc((void*)0x140807260,(void*)0x1410d6e88,count,gpuDesc);alloc((void*)0x140807260,(void*)0x1410d6de0,count,cpuDesc);D3D12_CPU_DESCRIPTOR_HANDLE a={gpuDesc[0]},b={cpuDesc[0]};D3D12_GPU_DESCRIPTOR_HANDLE g={gpuDesc[1]};auto step=host.d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
  uint32_t capacity=1;while(capacity<count*4)capacity*=2;table.resize(capacity*32);
  // Only the reader thread touches the catalog cursor. Decode the next group
  // while the render thread records uploads; bound lookahead by payload bytes.
  auto readBatch=[&](unsigned remaining){
   return std::async(std::launch::async,[&,remaining]{
    std::vector<OriginalD3DTexture> batch;
    size_t bytes=0;
    while(batch.size()<remaining && batch.size()<64 && bytes<(32u<<20)){
     batch.push_back(OriginalD3DTexture::read(input));
     bytes+=batch.back().payload.size();
    }
    return batch;
   });
  };
  auto next=readBatch(count);
  unsigned i=0;
  while(i<count){
   auto batch=next.get();
   unsigned remaining=count-i-static_cast<unsigned>(batch.size());
   if(remaining)next=readBatch(remaining);
   for(auto &texture:batch){
   textures.push_back(std::move(texture));auto &t=textures.back();if(t.deferred.empty())upload.append(t);else t.create(host);t.bind(host,{a.ptr+size_t(i)*step});t.bind(host,{b.ptr+size_t(i)*step});
   records.emplace_back(0x168);auto &r=records.back();if(!t.deferred.empty())pending.emplace(r.data()+0x68,i);auto put=[&](size_t off,auto value){memcpy(r.data()+off,&value,sizeof value);};
   put(0,uint64_t(t.name.size()));put(8,(uintptr_t)t.name.data());put(0x20,uint8_t(1));put(0x38,t.width);put(0x3c,t.height);put(0x40,uint32_t(1));put(0x44,uint16_t(t.mips));put(0x46,uint16_t(t.slices));put(0x48,sourceFormat(t.format));put(0x4a,uint16_t(t.flags));put(0x60,(uintptr_t)(r.data()+0x68));put(0x68,(uintptr_t)t.resource);put(0x78,uint32_t(D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE|D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE));
   #include "original_global_textures.inc"
   if(t.name=="black")*(void**)0x140b6d3a0=r.data();
   if(t.name=="black_texture")*(void**)0x140b6d2e8=r.data();
   put(0xc0,uint64_t(b.ptr+size_t(i)*step));put(0xd0,uint32_t(1));put(0xd4,uint32_t(cpuDesc[2]>>32)+i);put(0xd8,cpuDesc[3]);
   put(0xe0,uint64_t(a.ptr+size_t(i)*step));put(0xe8,uint64_t(g.ptr+size_t(i)*step));put(0xf0,uint32_t(1));put(0xf4,uint32_t(gpuDesc[2]>>32)+i);put(0xf8,gpuDesc[3]);
   if(t.name=="default_lightmap_array_texture"){memcpy((void*)0x14080cf18,r.data(),r.size());*(void**)0x14080cf78=(void*)0x14080cf80ull;}
   uint32_t hash=0x1505;for(unsigned char c:t.name)hash=(hash^c)*0x1b3;if(hash<2)hash|=2;uint32_t slot=hash&(capacity-1),increment=1;while(*(uint32_t*)(table.data()+slot*32)){slot=(slot+increment++)&(capacity-1);}auto entry=table.data()+slot*32;memcpy(entry,&hash,4);uint64_t fields[]={t.name.size(),(uintptr_t)t.name.data(),(uintptr_t)r.data()};memcpy(entry+8,fields,sizeof fields);
   ++i;
   }
  }
  upload.flush();*(uint64_t*)0x1410d7c78=count;*(uint64_t*)0x1410d7c80=capacity;*(uint64_t*)0x1410d7ca0=capacity;*(void**)0x1410d7ca8=table.data();fprintf(stderr,"Original texture catalog: %u assets, %zu deferred\n",count,pending.size());
 }
};
