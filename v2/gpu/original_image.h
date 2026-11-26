#pragma once
#include <sys/mman.h>
#include <fstream>
#include <vector>
#include <cstring>
#include <stdexcept>
#include <cstdint>
#include <CommonCrypto/CommonDigest.h>
// Maps PE sections only. Never runs entry point, TLS callbacks or initializers.
struct OriginalImage {
 uint8_t *base=nullptr;size_t size=0;
 template<class T> static T get(const std::vector<uint8_t>& b,size_t p){if(p+sizeof(T)>b.size())throw std::runtime_error("Truncated PE");T x;memcpy(&x,b.data()+p,sizeof x);return x;}
 OriginalImage(const char *path){
  std::ifstream f(path,std::ios::binary);if(!f)throw std::runtime_error("Cannot open original image");std::vector<uint8_t>b((std::istreambuf_iterator<char>(f)),{});
  uint8_t digest[CC_SHA256_DIGEST_LENGTH];CC_SHA256(b.data(),(CC_LONG)b.size(),digest);const char expected[]="69ddb71fb6768a097f9bff204201f3a9ce960f1a325f5ed3a5c9160dc64a6ae8";const char hex[]="0123456789abcdef";for(unsigned i=0;i<32;i++)if(hex[digest[i]>>4]!=expected[i*2]||hex[digest[i]&15]!=expected[i*2+1])throw std::runtime_error("Original executable hash mismatch");
  auto pe=get<uint32_t>(b,0x3c);if(get<uint16_t>(b,0)!=0x5a4d||get<uint32_t>(b,pe)!=0x4550||get<uint16_t>(b,pe+4)!=0x8664)throw std::runtime_error("Expected x64 PE");
  auto opt=pe+24;auto sections=get<uint16_t>(b,pe+6);auto table=opt+get<uint16_t>(b,pe+20);auto address=get<uint64_t>(b,opt+24);size=get<uint32_t>(b,opt+56);
  if(address!=0x140000000ull||size>256*1024*1024)throw std::runtime_error("Unexpected image layout");
  void *p=mmap((void*)address,size,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANON,-1,0);if(p==MAP_FAILED)throw std::runtime_error("Cannot map PE");if(p!=(void*)address){munmap(p,size);throw std::runtime_error("Original image address unavailable");}base=(uint8_t*)p;
  auto headers=get<uint32_t>(b,opt+60);if(headers>b.size()||headers>size)throw std::runtime_error("Invalid headers");memcpy(base,b.data(),headers);
  for(unsigned i=0;i<sections;i++){auto s=table+i*40;auto rva=get<uint32_t>(b,s+12),bytes=get<uint32_t>(b,s+16),offset=get<uint32_t>(b,s+20);if(uint64_t(rva)+bytes>size||uint64_t(offset)+bytes>b.size())throw std::runtime_error("Invalid section");memcpy(base+rva,b.data()+offset,bytes);}
  // Keep data writable, make only original executable sections executable.
  for(unsigned i=0;i<sections;i++){auto s=table+i*40;if(!(get<uint32_t>(b,s+36)&0x20000000))continue;auto rva=get<uint32_t>(b,s+12);auto n=get<uint32_t>(b,s+8);if(mprotect(base+rva,(n+4095)&~size_t(4095),PROT_READ|PROT_EXEC))throw std::runtime_error("Cannot protect original code");}
 }
 void patch(uint64_t address,const void *bytes,size_t n){auto page=address&~uint64_t(4095);auto length=((address+n-page)+4095)&~size_t(4095);if(mprotect((void*)page,length,PROT_READ|PROT_WRITE))throw std::runtime_error("Patch protection failed");memcpy((void*)address,bytes,n);if(mprotect((void*)page,length,PROT_READ|PROT_EXEC))throw std::runtime_error("Patch restore failed");}
 void redirect(uint64_t address,void *target){uint8_t jump[14]={0xff,0x25,0,0,0,0};memcpy(jump+6,&target,8);patch(address,jump,sizeof jump);}
 ~OriginalImage(){if(base)munmap(base,size);}
 template<class F> F function(uint64_t address){if(address<0x140000000ull||address>=0x140000000ull+size)throw std::runtime_error("Function outside image");return reinterpret_cast<F>(address);}
};
