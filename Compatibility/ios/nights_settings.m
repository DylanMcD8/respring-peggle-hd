#import <Foundation/Foundation.h>
// Read once before the guest starts; artwork selection lasts for this launch.
int peggle_show_nights_theme(void) {
    [NSUserDefaults.standardUserDefaults registerDefaults:@{@"PeggleShowNightsTheme":@NO}];
    return [NSUserDefaults.standardUserDefaults boolForKey:@"PeggleShowNightsTheme"] ? 1 : 0;
}
