// The parent owns the backing file. A worker keeps its mapping across draws;
// the line protocol serializes access between the renderer and the encoder.
#pragma once
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
struct FrameSlot {
 const size_t row,len;
 FrameSlot(size_t row,size_t height):row(row),len(row*height){}
 void *slots[2]={MAP_FAILED,MAP_FAILED};
 std::string names[2];
 ~FrameSlot(){for(void *p:slots)if(p!=MAP_FAILED)munmap(p,len);}
 void write(const char *path,const void *src,size_t pitch){
  unsigned i=0;while(i<2 && !names[i].empty() && names[i]!=path)++i;
  if(i==2)throw std::runtime_error("Unexpected third frame slot");
  void *&slot=slots[i];
  if(slot==MAP_FAILED){
   int fd=open(path,O_RDWR);struct stat st{};
   if(fd<0)throw std::runtime_error("Cannot open shared frame");
   if(fstat(fd,&st)==0&&st.st_size==off_t(len))slot=mmap(nullptr,len,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
   close(fd);names[i]=path;if(slot==MAP_FAILED)throw std::runtime_error("Cannot map shared frame");
  }
  if(row==pitch)memcpy(slot,src,len);
  else for(size_t y=0;y<len/row;++y)memcpy((char*)slot+y*row,(const char*)src+y*pitch,row);
 }
};
