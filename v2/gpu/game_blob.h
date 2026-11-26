// D3D resources are decoded on first use from the installed game, not cached
// into the program. Package extents are supplied by the Rust TOC reader.
#pragma once
#include <compression.h>
static Bytes sourceBytes(const std::string &path,uint64_t off=0,uint64_t size=0){
 std::ifstream f(path,std::ios::binary|std::ios::ate);
 if(!f)throw std::runtime_error("Cannot open game asset: "+path);
 uint64_t total=f.tellg();if(!size)size=total;if(off>total||size>total-off||size<12||size>(512ull<<20))throw std::runtime_error("Invalid game resource extent");
 Bytes buf(size);f.seekg(off);if(!f.read((char*)buf.data(),size))throw std::runtime_error("Truncated game asset");return buf;
}
static Bytes gamePayload(const Bytes &b){
 uint16_t codec,h;uint32_t n;memcpy(&codec,b.data()+4,2);memcpy(&h,b.data()+6,2);memcpy(&n,b.data()+8,4);
 if((h!=12&&h!=24&&h!=40)||h>b.size()||n<h||n>(512u<<20))throw std::runtime_error("Invalid game asset header");
 if(!codec&&b.size()==n)return Bytes(b.begin()+h,b.end());
 Bytes out(n-h);if(codec!=3||compression_decode_buffer(out.data(),out.size(),b.data()+h,b.size()-h,nullptr,COMPRESSION_LZ4_RAW)!=out.size())throw std::runtime_error("Game resource decode failed");return out;
}
