#pragma once
#include "d3dmetal_abi.h"
#include <dlfcn.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <memory>
#include <fstream>
#include <stdexcept>
#include <chrono>
#include <thread>
#include <algorithm>
using Bytes=std::vector<uint8_t>;
#include "game_blob.h"
// D3DMetal uses the explicit MS C++ struct-return ABI for these methods.
static D3D12_CPU_DESCRIPTOR_HANDLE cpuHandle(ID3D12DescriptorHeap *h){
 using Fn=D3D12_CPU_DESCRIPTOR_HANDLE* (STDMETHODCALLTYPE *)(ID3D12DescriptorHeap*,D3D12_CPU_DESCRIPTOR_HANDLE*);
 D3D12_CPU_DESCRIPTOR_HANDLE r={};((Fn)(*(void***)h)[9])(h,&r);return r;
}
static D3D12_GPU_DESCRIPTOR_HANDLE gpuHandle(ID3D12DescriptorHeap *h){
 using Fn=D3D12_GPU_DESCRIPTOR_HANDLE* (STDMETHODCALLTYPE *)(ID3D12DescriptorHeap*,D3D12_GPU_DESCRIPTOR_HANDLE*);
 D3D12_GPU_DESCRIPTOR_HANDLE r={};((Fn)(*(void***)h)[10])(h,&r);return r;
}
static void check(HRESULT hr,const char *what){if(hr<0){char b[200];snprintf(b,sizeof b,"%s: HRESULT 0x%08x",what,(unsigned)hr);throw std::runtime_error(b);}}
struct Reader {
 std::ifstream f;Reader(const char *p):f(p,std::ios::binary){if(!f)throw std::runtime_error(std::string("Cannot open scene: ")+p);}
 uint32_t word(){uint32_t v;f.read((char*)&v,4);if(!f)throw std::runtime_error("Truncated scene");return v;}
 Bytes blob(std::string* deferred=nullptr,bool deferAll=false){uint32_t n=word();if(n>=UINT32_MAX-1){auto bytes=blob();std::string path(bytes.begin(),bytes.end());if(deferred&&(deferAll||n==UINT32_MAX-1)){*deferred=path;return {};}return gamePayload(sourceBytes(path));}if(n>512*1024*1024)throw std::runtime_error("Invalid blob size");Bytes b(n);f.read((char*)b.data(),n);if(!f)throw std::runtime_error("Truncated blob");return b;}
};
// Concatenate catalog records without copying shared assets into every level bundle.
struct BundleReader {
 std::vector<std::string> paths;std::unique_ptr<Reader> current;size_t index=0;uint32_t count=0;unsigned prefix=0;bool texture;
 BundleReader(const char*path,bool tex):texture(tex){std::string name(path);if(name.size()>=5&&name.substr(name.size()-5)==".list"){std::ifstream f(path);std::string line;while(std::getline(f,line))if(!line.empty())paths.push_back(line);}else paths.push_back(path);for(auto&p:paths){Reader r(p.c_str());if(texture&&r.word()!=0x31545844)throw std::runtime_error("Invalid texture shard");count+=r.word();}if(paths.empty())throw std::runtime_error("Empty catalog list");open();}
 void open(){current=std::make_unique<Reader>(paths.at(index).c_str());if(texture)current->word();current->word();}
 Reader&data(){while(current->f.peek()==EOF&&index+1<paths.size()){index++;open();}return *current;}
 uint32_t word(){if(texture&&prefix==0){prefix++;return 0x31545844;}if(prefix<(texture?2u:1u)){prefix++;return count;}return data().word();}
 Bytes blob(std::string* deferred=nullptr,bool deferAll=false){return data().blob(deferred,deferAll);}
};
struct Host {
 void *lib;ID3D12Device *d=nullptr;std::vector<IUnknown*> objects;
 template<class T> const IID &iid(){return __uuidof(T);}
 template<class T> T* keep(T* o){objects.push_back(o);return o;}
 Host(const char *path){lib=dlopen(path,RTLD_NOW|RTLD_LOCAL);if(!lib)throw std::runtime_error(dlerror());
  using Create=HRESULT (WINAPI *)(IUnknown*,D3D_FEATURE_LEVEL,REFIID,void**);
  auto create=(Create)dlsym(lib,"D3D12CreateDevice");if(!create)throw std::runtime_error("Missing device entry");
  check(create(nullptr,D3D_FEATURE_LEVEL_11_0,iid<ID3D12Device>(),(void**)&d),"CreateDevice");}
 ~Host(){for(auto it=objects.rbegin();it!=objects.rend();++it)(*it)->Release();if(d)d->Release();}
 ID3D12Resource* resource(const D3D12_RESOURCE_DESC &desc,D3D12_HEAP_TYPE type,D3D12_RESOURCE_STATES state,const D3D12_CLEAR_VALUE *clear=nullptr){
  D3D12_HEAP_PROPERTIES heap={};heap.Type=type;heap.CreationNodeMask=heap.VisibleNodeMask=1;ID3D12Resource *r=nullptr;
  check(d->CreateCommittedResource(&heap,D3D12_HEAP_FLAG_NONE,&desc,state,clear,iid<ID3D12Resource>(),(void**)&r),"CreateCommittedResource");return keep(r);}
 ID3D12Resource* buffer(size_t size,D3D12_HEAP_TYPE type=D3D12_HEAP_TYPE_UPLOAD){D3D12_RESOURCE_DESC desc={};desc.Dimension=D3D12_RESOURCE_DIMENSION_BUFFER;desc.Width=std::max(size,size_t(256));desc.Height=1;desc.DepthOrArraySize=1;desc.MipLevels=1;desc.SampleDesc.Count=1;desc.Layout=D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
  return resource(desc,type,type==D3D12_HEAP_TYPE_READBACK?D3D12_RESOURCE_STATE_COPY_DEST:D3D12_RESOURCE_STATE_GENERIC_READ);}
 ID3D12Resource* upload(const Bytes &b){auto r=buffer((b.size()+255)&~size_t(255));void *p;D3D12_RANGE none={0,0};check(r->Map(0,&none,&p),"Map upload");memcpy(p,b.data(),b.size());D3D12_RANGE written={0,b.size()};r->Unmap(0,&written);return r;}
 ID3D12DescriptorHeap* heap(D3D12_DESCRIPTOR_HEAP_TYPE type,UINT count,bool visible){D3D12_DESCRIPTOR_HEAP_DESC desc={};desc.Type=type;desc.NumDescriptors=count;desc.Flags=visible?D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE:D3D12_DESCRIPTOR_HEAP_FLAG_NONE;ID3D12DescriptorHeap *h=nullptr;check(d->CreateDescriptorHeap(&desc,iid<ID3D12DescriptorHeap>(),(void**)&h),"CreateDescriptorHeap");return keep(h);}
};
