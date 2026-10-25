#import <Foundation/Foundation.h>
#include <math.h>
#include <TargetConditionals.h>
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
void peggle_wrapper_native_pixels(unsigned int *width,unsigned int *height){
 UIScreen *screen=nil;
 for(UIScene *scene in UIApplication.sharedApplication.connectedScenes){if([scene isKindOfClass:UIWindowScene.class]&&scene.activationState==UISceneActivationStateForegroundActive){screen=((UIWindowScene *)scene).screen;break;}}
 if(!screen)screen=UIScreen.mainScreen;
 CGSize pixels=screen.nativeBounds.size;*width=(unsigned int)llround(pixels.width);*height=(unsigned int)llround(pixels.height);
}
#endif

#if TARGET_OS_IOS
#include <SDL.h>
#include <SDL_syswm.h>
#import <OpenGLES/EAGL.h>
#import <OpenGLES/ES1/gl.h>
#import <OpenGLES/ES1/glext.h>
#include "wrapper_backing_query.h"
extern int SDL_UIKitBindCurrentDrawable(void);
static void backing_bindings(unsigned int *f,unsigned int *r){
 GLint fb=0,rb=0;glGetIntegerv(GL_FRAMEBUFFER_BINDING_OES,&fb);glGetIntegerv(GL_RENDERBUFFER_BINDING_OES,&rb);*f=(unsigned int)fb;*r=(unsigned int)rb;
}
static int backing_bind_current(void *window){
 if(SDL_GL_GetCurrentWindow()!=(SDL_Window *)window)return -1;
 return SDL_UIKitBindCurrentDrawable();
}
static void backing_size(int *w,int *h){
 glGetRenderbufferParameterivOES(GL_RENDERBUFFER_OES,GL_RENDERBUFFER_WIDTH_OES,w);
 glGetRenderbufferParameterivOES(GL_RENDERBUFFER_OES,GL_RENDERBUFFER_HEIGHT_OES,h);
}
static void backing_restore(unsigned int f,unsigned int r){
 glBindFramebufferOES(GL_FRAMEBUFFER_OES,f);glBindRenderbufferOES(GL_RENDERBUFFER_OES,r);
}
// Called after making the host context current, before guest contexts exist.
// WMInfo supplies window/screen metadata only: its colorbuffer belongs to the
// permanent root view, which owns the keyboard and is deliberately not a GL view.
int peggle_wrapper_backing_metrics(void *sdl_window,unsigned int *width,unsigned int *height,
                                  unsigned int *bounds_width,unsigned int *bounds_height,double *native_scale,
                                  int *query_status,unsigned int *root_colorbuffer){
 *width=*height=*bounds_width=*bounds_height=*root_colorbuffer=0;*native_scale=0;
 SDL_SysWMinfo info;SDL_VERSION(&info.version);
 if(SDL_GetWindowWMInfo((SDL_Window *)sdl_window,&info)&&info.subsystem==SDL_SYSWM_UIKIT){
  *root_colorbuffer=info.info.uikit.colorbuffer;
  UIWindow *window=info.info.uikit.window;UIScreen *screen=window.screen;
  if(screen){CGSize pixels=screen.nativeBounds.size;*bounds_width=(unsigned int)llround(pixels.width);*bounds_height=(unsigned int)llround(pixels.height);*native_scale=screen.nativeScale;}
 }
 SDL_GLContext current=SDL_GL_GetCurrentContext();
 int matches=current&&SDL_GL_GetCurrentWindow()==(SDL_Window *)sdl_window&&(__bridge void *)[EAGLContext currentContext]==current;
 const PeggleBackingOps ops={backing_bindings,backing_bind_current,backing_size,backing_restore};
 *query_status=peggle_query_current_backing(sdl_window,matches,&ops,width,height);
 return *query_status==PEGGLE_BACKING_VERIFIED;
}
#endif
