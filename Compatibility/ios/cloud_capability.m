#import "cloud_capability.h"
#import <mach-o/loader.h>
#import <libkern/OSByteOrder.h>
#include <string.h>

static BOOL Range(NSUInteger length, NSUInteger offset, NSUInteger count) {
 return offset<=length && count<=length-offset;
}
static uint32_t Big32(const uint8_t *p) {uint32_t n;memcpy(&n,p,4);return OSSwapBigToHostInt32(n);}
NSDictionary *PeggleSignedEntitlements(NSString *executable) {
 if(!executable.length)return nil;
 NSData *file=[NSData dataWithContentsOfFile:executable options:NSDataReadingMappedIfSafe error:NULL];
 if(file.length<sizeof(struct mach_header_64))return nil;
 const uint8_t *bytes=file.bytes;struct mach_header_64 header;memcpy(&header,bytes,sizeof(header));
 // These apps ship a thin ARM64 executable. An unknown layout must not make
 // an ordinary sideload look CloudKit-capable.
 if(header.magic!=MH_MAGIC_64 || header.cputype!=CPU_TYPE_ARM64 ||
    !Range(file.length,sizeof(header),header.sizeofcmds) || header.ncmds>header.sizeofcmds/sizeof(struct load_command))return nil;
 NSUInteger offset=sizeof(header),end=offset+header.sizeofcmds;
 for(uint32_t i=0;i<header.ncmds;i++) {
  if(!Range(end,offset,sizeof(struct load_command)))return nil;
  struct load_command command;memcpy(&command,bytes+offset,sizeof(command));
  if(command.cmdsize<sizeof(command)||!Range(end,offset,command.cmdsize))return nil;
  if(command.cmd==LC_CODE_SIGNATURE) {
   if(command.cmdsize<sizeof(struct linkedit_data_command))return nil;
   struct linkedit_data_command signature;memcpy(&signature,bytes+offset,sizeof(signature));
   if(signature.datasize<12||!Range(file.length,signature.dataoff,signature.datasize))return nil;
   const uint8_t *blob=bytes+signature.dataoff;
   if(Big32(blob)!=0xfade0cc0)return nil; // embedded-signature SuperBlob
   NSUInteger length=Big32(blob+4),count=Big32(blob+8);
   if(length<12||length>signature.datasize||count>(length-12)/8)return nil;
   for(NSUInteger j=0;j<count;j++) {
    if(Big32(blob+12+j*8)!=5)continue; // CSSLOT_ENTITLEMENTS (XML plist)
    NSUInteger start=Big32(blob+16+j*8);
    if(!Range(length,start,8)||Big32(blob+start)!=0xfade7171)return nil;
    NSUInteger size=Big32(blob+start+4);
    if(size<8||size>1024*1024||!Range(length,start,size))return nil;
    NSData *payload=[NSData dataWithBytes:blob+start+8 length:size-8];
    id value=[NSPropertyListSerialization propertyListWithData:payload options:NSPropertyListImmutable format:NULL error:NULL];
    return [value isKindOfClass:NSDictionary.class]?value:nil;
   }
   return nil;
  }
  offset+=command.cmdsize;
 }
 return nil;
}
BOOL PeggleCloudEntitlementsAuthorize(NSDictionary *entitlements, NSString *container) {
 id services=entitlements[@"com.apple.developer.icloud-services"];
 id containers=entitlements[@"com.apple.developer.icloud-container-identifiers"];
 id environment=entitlements[@"com.apple.developer.icloud-container-environment"];
 return container.length && [services isKindOfClass:NSArray.class] && [services containsObject:@"CloudKit"] &&
  [containers isKindOfClass:NSArray.class] && [containers containsObject:container] &&
  [environment isKindOfClass:NSString.class] && ([@"Development" isEqual:environment]||[@"Production" isEqual:environment]);
}
BOOL PeggleCloudSupported(void) {
 static BOOL supported;static dispatch_once_t once;
 dispatch_once(&once,^{
  NSBundle *bundle=NSBundle.mainBundle;
  if(![[bundle objectForInfoDictionaryKey:@"RespringProgressCloudKit"]boolValue])return;
  NSString *container=[bundle objectForInfoDictionaryKey:@"RespringCloudKitContainer"]?:@"iCloud.com.respringapps.peggle";
  supported=PeggleCloudEntitlementsAuthorize(PeggleSignedEntitlements(bundle.executablePath),container);
 });
 return supported;
}
