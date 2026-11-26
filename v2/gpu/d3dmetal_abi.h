#pragma once
#include <basetsd.h>
#undef STDMETHODCALLTYPE
#undef WINAPI
#undef __stdcall
#define STDMETHODCALLTYPE __attribute__((ms_abi))
#define WINAPI __attribute__((ms_abi))
#define __stdcall __attribute__((ms_abi))
#include <unknwn.h>
#include <directx/d3d12.h>
#include <dxguids/dxguids.h>
