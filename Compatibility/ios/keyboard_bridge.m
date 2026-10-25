#import <UIKit/UIKit.h>
#include <stdint.h>
#include <stdbool.h>

typedef struct {
    int32_t phase;
    float begin[4], end[4];
    double duration;
    int32_t curve;
} PeggleKeyboardEvent;
static PeggleKeyboardEvent queue[16];
static unsigned head, count;
static NSMutableArray *observerTokens;

static void normalizedFrame(CGRect frame, UIWindow *window, float out[4]) {
    CGRect local = [window convertRect:frame fromCoordinateSpace:window.screen.coordinateSpace];
    CGRect bounds = window.bounds;
    if (bounds.size.width <= 0 || bounds.size.height <= 0) return;
    out[0] = (local.origin.x - bounds.origin.x) / bounds.size.width;
    out[1] = (local.origin.y - bounds.origin.y) / bounds.size.height;
    out[2] = local.size.width / bounds.size.width;
    out[3] = local.size.height / bounds.size.height;
}

void peggle_install_keyboard_bridge(void) {
    observerTokens = [NSMutableArray new];
    NSArray *names = @[UIKeyboardWillShowNotification, UIKeyboardDidShowNotification,
                       UIKeyboardWillHideNotification, UIKeyboardDidHideNotification];
    for (unsigned i=0; i<names.count; ++i) {
        int32_t phase = (int32_t)i + 1;
        id token = [NSNotificationCenter.defaultCenter addObserverForName:names[i] object:nil
                    queue:nil usingBlock:^(NSNotification *note) {
            UIWindow *window = nil;
            for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
                if (![scene isKindOfClass:UIWindowScene.class]) continue;
                for (UIWindow *candidate in ((UIWindowScene *)scene).windows)
                    if (candidate.isKeyWindow) window = candidate;
            }
            if (!window) return;
            PeggleKeyboardEvent event = {0};
            event.phase = phase;
            normalizedFrame([note.userInfo[UIKeyboardFrameBeginUserInfoKey] CGRectValue], window, event.begin);
            normalizedFrame([note.userInfo[UIKeyboardFrameEndUserInfoKey] CGRectValue], window, event.end);
            event.duration = [note.userInfo[UIKeyboardAnimationDurationUserInfoKey] doubleValue];
            event.curve = [note.userInfo[UIKeyboardAnimationCurveUserInfoKey] intValue];
            if (count == 16) { head = (head+1)%16; --count; }
            queue[(head+count)%16] = event; ++count;
            NSLog(@"Peggle keyboard event %d; native window %@; begin %@; end %@",
                  phase, NSStringFromCGRect(window.bounds),
                  NSStringFromCGRect([note.userInfo[UIKeyboardFrameBeginUserInfoKey] CGRectValue]),
                  NSStringFromCGRect([note.userInfo[UIKeyboardFrameEndUserInfoKey] CGRectValue]));
        }];
        [observerTokens addObject:token];
    }
}

// Native UIKit and the emulator host poll both run on the main OS thread.
bool peggle_poll_keyboard_event(PeggleKeyboardEvent *event) {
    if (count == 0) return false;
    *event = queue[head]; head = (head+1)%16; --count;
    return true;
}
