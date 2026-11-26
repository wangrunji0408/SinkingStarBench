// Small system-framework adapter. Frame selection, HUD timing and rendering
// decisions are made in Rust. AVAssetWriter owns the VideoToolbox HEVC session.
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <Accelerate/Accelerate.h>
#import <ImageIO/ImageIO.h>
#include <unistd.h>
@interface StarWriter:NSObject
@property(strong) AVAssetWriter *writer;
@property(strong) AVAssetWriterInput *input;
@property(strong) AVAssetWriterInputPixelBufferAdaptor *pool;
@property(nonatomic) uint32_t width, height, fps;
@end
@implementation StarWriter
@end
static int err(NSString *s){fprintf(stderr,"VideoToolbox: %s\n",s.UTF8String);return 0;}
void* star_video_open(const char *path,uint32_t width,uint32_t height,uint32_t fps,uint8_t quality){@autoreleasepool{
 NSError*e=nil;StarWriter *v=[StarWriter new];v.writer=[[AVAssetWriter alloc]initWithURL:[NSURL fileURLWithPath:@(path)] fileType:AVFileTypeMPEG4 error:&e];if(!v.writer){err(e.description);return NULL;}
 v.width=width;v.height=height;v.fps=fps;
 NSDictionary *compression=@{AVVideoQualityKey:@(quality/100.), AVVideoExpectedSourceFrameRateKey:@(fps)};
 v.writer.shouldOptimizeForNetworkUse=YES;v.input=[AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeVideo outputSettings:@{AVVideoCodecKey:AVVideoCodecTypeHEVC,AVVideoWidthKey:@(width),AVVideoHeightKey:@(height),AVVideoCompressionPropertiesKey:compression}];v.input.mediaTimeScale=600;v.pool=[AVAssetWriterInputPixelBufferAdaptor assetWriterInputPixelBufferAdaptorWithAssetWriterInput:v.input sourcePixelBufferAttributes:@{(id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32BGRA),(id)kCVPixelBufferWidthKey:@(width),(id)kCVPixelBufferHeightKey:@(height),(id)kCVPixelBufferIOSurfacePropertiesKey:@{}}];
 if(![v.writer canAddInput:v.input])return NULL;[v.writer addInput:v.input];if(![v.writer startWriting]){err(v.writer.error.description);return NULL;}[v.writer startSessionAtSourceTime:kCMTimeZero];return (__bridge_retained void*)v;
}}
int star_video_frame(void *ptr,const void*rgba,uint32_t i,const char*labels){@autoreleasepool{StarWriter*v=(__bridge StarWriter*)ptr;
 for(int n=0;!v.input.readyForMoreMediaData;n++){if(v.writer.status!=AVAssetWriterStatusWriting||n>120000)return err(v.writer.error.description?:@"encoder timeout");usleep(1000);}
 CVPixelBufferRef pixel=NULL;if(CVPixelBufferPoolCreatePixelBuffer(NULL,v.pool.pixelBufferPool,&pixel)!=kCVReturnSuccess)return err(@"pixel buffer allocation");
 CVPixelBufferLockBaseAddress(pixel,0);void*dst=CVPixelBufferGetBaseAddress(pixel);size_t stride=CVPixelBufferGetBytesPerRow(pixel);
 vImage_Buffer a={(void*)rgba,v.height,v.width,v.width*4},b={dst,v.height,v.width,stride};uint8_t map[]={2,1,0,3};vImagePermuteChannels_ARGB8888(&a,&b,map,kvImageNoFlags);
 if(*labels){CGColorSpaceRef rgb=CGColorSpaceCreateDeviceRGB();CGContextRef cg=CGBitmapContextCreate(dst,v.width,v.height,8,stride,rgb,kCGImageAlphaPremultipliedFirst|kCGBitmapByteOrder32Little);CGColorSpaceRelease(rgb);[NSGraphicsContext saveGraphicsState];[NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithCGContext:cg flipped:NO]];CGFloat scale=MIN(v.width/1280.,v.height/720.);CGContextScaleCTM(cg,scale,scale);CGFloat right=v.width/scale-26;
 for(NSString*k in [[@(labels) componentsSeparatedByString:@" "] reverseObjectEnumerator]){NSDictionary*attr=@{NSFontAttributeName:[NSFont systemFontOfSize:k.length==1?29:21],NSForegroundColorAttributeName:NSColor.whiteColor};NSSize size=[k sizeWithAttributes:attr];CGFloat w=MAX(42,ceil(size.width)+16);NSRect rect=NSMakeRect(right-w,26,w,49);NSBezierPath*p=[NSBezierPath bezierPathWithRoundedRect:rect xRadius:9 yRadius:9];[[NSColor colorWithWhite:.07 alpha:.84]setFill];[p fill];[[NSColor colorWithWhite:1 alpha:.75]setStroke];[p stroke];[k drawAtPoint:NSMakePoint(NSMidX(rect)-size.width/2,NSMidY(rect)-size.height/2) withAttributes:attr];right-=w+6;}[NSGraphicsContext restoreGraphicsState];CGContextRelease(cg);}
 CVPixelBufferUnlockBaseAddress(pixel,0);BOOL ok=[v.pool appendPixelBuffer:pixel withPresentationTime:CMTimeMake(i,v.fps)];CVPixelBufferRelease(pixel);return ok?1:err(v.writer.error.description);
}}
int star_video_finish(void *ptr,uint32_t n){@autoreleasepool{StarWriter*v=(__bridge StarWriter*)ptr;[v.writer endSessionAtSourceTime:CMTimeMake(n,v.fps)];[v.input markAsFinished];dispatch_semaphore_t sem=dispatch_semaphore_create(0);[v.writer finishWritingWithCompletionHandler:^{dispatch_semaphore_signal(sem);}];if(dispatch_semaphore_wait(sem,dispatch_time(DISPATCH_TIME_NOW,180LL*NSEC_PER_SEC)))return err(@"finalize timeout");return v.writer.status==AVAssetWriterStatusCompleted?1:err(v.writer.error.description);}}
void star_video_close(void*p){@autoreleasepool{StarWriter*v=CFBridgingRelease(p);if(v.writer.status==AVAssetWriterStatusWriting)[v.writer cancelWriting];}}
int64_t star_video_probe(const char*path,uint32_t width,uint32_t height,uint32_t fps){@autoreleasepool{AVURLAsset*a=[AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:@(path)] options:@{AVURLAssetPreferPreciseDurationAndTimingKey:@YES}];AVAssetTrack*t=[a tracksWithMediaType:AVMediaTypeVideo].firstObject;if(!t||t.naturalSize.width!=width||t.naturalSize.height!=height)return -1;CMFormatDescriptionRef fmt=(__bridge CMFormatDescriptionRef)t.formatDescriptions.firstObject;if(!fmt||CMFormatDescriptionGetMediaSubType(fmt)!=kCMVideoCodecType_HEVC)return -1;NSError*e=nil;AVAssetReader*r=[[AVAssetReader alloc]initWithAsset:a error:&e];AVAssetReaderTrackOutput*out=[AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:t outputSettings:nil];if(![r canAddOutput:out])return -1;[r addOutput:out];if(![r startReading])return -1;int64_t n=0;CMSampleBufferRef s;while((s=[out copyNextSampleBuffer])){n+=CMSampleBufferGetNumSamples(s);CFRelease(s);}return r.status==AVAssetReaderStatusCompleted&&CMTimeCompare(t.timeRange.duration,CMTimeMake(n,fps))==0?n:-1;}}

int star_image_write(const char *path, const void *rgba, int jpeg, uint32_t width, uint32_t height) {
    @autoreleasepool {
        CGColorSpaceRef rgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGDataProviderRef pixels = CGDataProviderCreateWithData(NULL, rgba, (size_t)width * height * 4, NULL);
        CGImageRef image = pixels && rgb ? CGImageCreate(
            width, height, 8, 32, width * 4, rgb,
            kCGBitmapByteOrder32Big | kCGImageAlphaNoneSkipLast, pixels,
            NULL, false, kCGRenderingIntentDefault) : NULL;
        if (pixels) CGDataProviderRelease(pixels);
        if (rgb) CGColorSpaceRelease(rgb);
        if (!image) return 0;

        NSMutableData *data = [NSMutableData data];
        CGImageDestinationRef dst = CGImageDestinationCreateWithData(
            (__bridge CFMutableDataRef)data, jpeg ? CFSTR("public.jpeg") : CFSTR("public.png"), 1, NULL);
        BOOL ok = NO;
        if (dst) {
            NSDictionary *options = jpeg ? @{(__bridge NSString *)kCGImageDestinationLossyCompressionQuality: @0.95} : nil;
            CGImageDestinationAddImage(dst, image, (__bridge CFDictionaryRef)options);
            ok = CGImageDestinationFinalize(dst);
            CFRelease(dst);
        }
        CGImageRelease(image);
        if (!ok) return 0;

        NSError *error = nil;
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        if ([data writeToURL:url options:NSDataWritingAtomic error:&error]) return 1;
        fprintf(stderr, "ImageIO: %s\n", error.localizedDescription.UTF8String);
        return 0;
    }
}
