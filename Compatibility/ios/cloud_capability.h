#import <Foundation/Foundation.h>
// Reads the executable's signed entitlement blob, not its provisioning profile.
// Unsupported/malformed signatures fail closed. No private Security API is used.
NSDictionary *PeggleSignedEntitlements(NSString *executable);
BOOL PeggleCloudEntitlementsAuthorize(NSDictionary *entitlements, NSString *container);
BOOL PeggleCloudSupported(void);
