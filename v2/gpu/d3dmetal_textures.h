#pragma once
#include "d3dmetal_host.h"
#include <string>
struct CommandBatch {
 uint64_t fenceValue=0;
 Host &host;ID3D12CommandQueue *queue;ID3D12CommandAllocator *allocator;ID3D12GraphicsCommandList *list;ID3D12Fence *fence;
 CommandBatch(Host &h):host(h){auto d=h.d;D3D12_COMMAND_QUEUE_DESC q={};q.Type=D3D12_COMMAND_LIST_TYPE_DIRECT;
  check(d->CreateCommandQueue(&q,h.iid<ID3D12CommandQueue>(),(void**)&queue),"Create upload queue");h.keep(queue);
  check(d->CreateCommandAllocator(q.Type,h.iid<ID3D12CommandAllocator>(),(void**)&allocator),"Create upload allocator");h.keep(allocator);
  check(d->CreateCommandList(0,q.Type,allocator,nullptr,h.iid<ID3D12GraphicsCommandList>(),(void**)&list),"Create upload list");h.keep(list);
  check(d->CreateFence(0,D3D12_FENCE_FLAG_NONE,h.iid<ID3D12Fence>(),(void**)&fence),"Create upload fence");h.keep(fence);
 }
 void reset(){check(allocator->Reset(),"Reset allocator");check(list->Reset(allocator,nullptr),"Reset command list");}
 void wait(){check(list->Close(),"Close upload list");ID3D12CommandList *p=list;queue->ExecuteCommandLists(1,&p);check(queue->Signal(fence,++fenceValue),"Signal upload fence");auto start=std::chrono::steady_clock::now();while(fence->GetCompletedValue()<fenceValue){if(std::chrono::steady_clock::now()-start>std::chrono::seconds(30))throw std::runtime_error("Upload timeout");std::this_thread::sleep_for(std::chrono::milliseconds(1));}check(host.d->GetDeviceRemovedReason(),"Upload device status");}
};
struct TextureSubresource {UINT index,offset,width,height,pitch,rowBytes,rows;};
struct OriginalD3DTexture {
 std::string name,deferred;size_t minimumBytes=0;DXGI_FORMAT format;UINT width,height,slices,mips,flags;Bytes payload;std::vector<TextureSubresource> subs;ID3D12Resource *resource=nullptr;
 template<class R> static OriginalD3DTexture read(R &r){OriginalD3DTexture t;auto name=r.blob();t.name=std::string(name.begin(),name.end());t.format=(DXGI_FORMAT)r.word();t.width=r.word();t.height=r.word();t.slices=r.word();t.mips=r.word();t.flags=r.word();UINT count=r.word();t.payload=r.blob(&t.deferred);
  if(!t.width||!t.height||!t.slices||!t.mips||t.mips>16||t.slices>65535||count!=t.slices*t.mips)throw std::runtime_error("Invalid texture dimensions");
  std::vector<bool> indices(count,false);
  for(UINT i=0;i<count;i++){TextureSubresource s={r.word(),r.word(),r.word(),r.word(),r.word(),r.word(),r.word()};
   if(s.index>=count||indices[s.index]||s.offset%512||s.pitch%256||!s.rows||!s.rowBytes||s.rowBytes>s.pitch||size_t(s.offset)+size_t(s.pitch)*s.rows>(t.deferred.empty()?t.payload.size():size_t(512)<<20)||s.width!=std::max(1u,t.width>>(s.index%t.mips))||s.height!=std::max(1u,t.height>>(s.index%t.mips)))throw std::runtime_error("Invalid texture footprint");t.minimumBytes=std::max(t.minimumBytes,size_t(s.offset)+size_t(s.pitch)*s.rows);indices[s.index]=true;t.subs.push_back(s);}
  return t;
 }
 void transition(ID3D12GraphicsCommandList *list,D3D12_RESOURCE_STATES before,D3D12_RESOURCE_STATES after){D3D12_RESOURCE_BARRIER b={};b.Type=D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;b.Transition={resource,D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,before,after};list->ResourceBarrier(1,&b);}
 void create(Host& host){
  D3D12_RESOURCE_DESC d={};d.Dimension=D3D12_RESOURCE_DIMENSION_TEXTURE2D;d.Width=width;d.Height=height;d.DepthOrArraySize=UINT16(slices);d.MipLevels=UINT16(mips);d.Format=format;d.SampleDesc.Count=1;
  resource=host.resource(d,D3D12_HEAP_TYPE_DEFAULT,D3D12_RESOURCE_STATE_COPY_DEST);
 }
 void upload(Host &host,ID3D12GraphicsCommandList *list,ID3D12Resource *staging,size_t base){
  if(!resource)create(host);
  for(auto s:subs){D3D12_TEXTURE_COPY_LOCATION src={},dst={};src.pResource=staging;src.Type=D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;src.PlacedFootprint.Offset=base+s.offset;src.PlacedFootprint.Footprint={format,s.width,s.height,1,s.pitch};dst.pResource=resource;dst.Type=D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;dst.SubresourceIndex=s.index;list->CopyTextureRegion(&dst,0,0,0,&src,nullptr);}
  transition(list,D3D12_RESOURCE_STATE_COPY_DEST,D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE|D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);
 }
 ID3D12Resource *readback(Host &host,ID3D12GraphicsCommandList *list){auto target=host.buffer(payload.size(),D3D12_HEAP_TYPE_READBACK);transition(list,D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE|D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,D3D12_RESOURCE_STATE_COPY_SOURCE);
  for(auto s:subs){D3D12_TEXTURE_COPY_LOCATION src={},dst={};src.pResource=resource;src.Type=D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;src.SubresourceIndex=s.index;dst.pResource=target;dst.Type=D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;dst.PlacedFootprint.Offset=s.offset;dst.PlacedFootprint.Footprint={format,s.width,s.height,1,s.pitch};list->CopyTextureRegion(&dst,0,0,0,&src,nullptr);}
  transition(list,D3D12_RESOURCE_STATE_COPY_SOURCE,D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE|D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);return target;
 }
 void bind(Host &host,D3D12_CPU_DESCRIPTOR_HANDLE handle){D3D12_SHADER_RESOURCE_VIEW_DESC d={};d.Format=format;d.Shader4ComponentMapping=D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
  if(flags&2){if(flags&4){d.ViewDimension=D3D12_SRV_DIMENSION_TEXTURECUBEARRAY;d.TextureCubeArray.MipLevels=mips;d.TextureCubeArray.NumCubes=slices/6;}else{d.ViewDimension=D3D12_SRV_DIMENSION_TEXTURECUBE;d.TextureCube.MipLevels=mips;}}
  else if(flags&4){d.ViewDimension=D3D12_SRV_DIMENSION_TEXTURE2DARRAY;d.Texture2DArray.MipLevels=mips;d.Texture2DArray.ArraySize=slices;}
  else {d.ViewDimension=D3D12_SRV_DIMENSION_TEXTURE2D;d.Texture2D.MipLevels=mips;}
  host.d->CreateShaderResourceView(resource,&d,handle);
 }
 void verify(ID3D12Resource *target){void *p;D3D12_RANGE range={0,payload.size()};check(target->Map(0,&range,&p),"Map texture readback");
  for(auto s:subs)for(UINT row=0;row<s.rows;row++){size_t offset=s.offset+size_t(row)*s.pitch;if(memcmp((char*)p+offset,payload.data()+offset,s.rowBytes))throw std::runtime_error("Original texture bytes differ: "+name);}
  D3D12_RANGE none={0,0};target->Unmap(0,&none);
 }
};
