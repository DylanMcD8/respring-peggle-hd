#import <Foundation/Foundation.h>
BOOL PeggleUsesFolderSync(void);
BOOL PeggleSyncSupported(void);
BOOL PeggleSyncReady(void);
NSString *PeggleSyncPreferenceKey(NSString *key);
NSString *PeggleSyncContext(void);
NSString *PeggleFolderName(void);
void PeggleSelectSyncFolder(NSURL *url, void(^completion)(NSError *));
void PeggleFolderExchange(NSDictionary *source, NSDictionary *(^merge)(NSDictionary *), void(^completion)(NSDictionary *));
// Coordinated I/O core, also exercised with local directories in native tests.
NSDictionary *PeggleFolderExchangeAtURL(NSURL *folder, NSString *writer, NSDictionary *source, NSDictionary *(^merge)(NSDictionary *));
