//
//  RCTCyfsStack.m
//  LianYin
//
//  Created by bucky on 2021/7/24.
//  Copyright © 2021 Facebook. All rights reserved.
//

#import <Foundation/Foundation.h>
#import "RCTCyfsStack.h"
#import "stack.h"

#include <ifaddrs.h>
#include <arpa/inet.h>

@implementation RCTCyfsStack

RCT_EXPORT_MODULE(CyfsStack)

static NSString* getWifiIpv4Addr() {
  NSString *address = @"";
  struct ifaddrs *interfaces = NULL;
  struct ifaddrs *temp_addr = NULL;
  int success = getifaddrs(&interfaces);
  
  if (success == 0) {
    temp_addr = interfaces;
    while (temp_addr) {
      if ( temp_addr->ifa_addr->sa_family == AF_INET)
      {
        // Check if interface is en0 which is the wifi connection on the iPhone
        if (strcmp(temp_addr->ifa_name, "en0") == 0) {
          // Get NSString from C String
          address = [NSString stringWithUTF8String:inet_ntoa(((struct sockaddr_in *)temp_addr->ifa_addr)->sin_addr)];
          NSLog(@"get wifi ipv4 addr %@", address);
          break;
        }
      }
      temp_addr = temp_addr->ifa_next;
    }
  }
  
  freeifaddrs(interfaces);
  return address;
}

static void log_c(const uint8_t* msg) {
  NSLog(@"%s", msg);
}

RCT_EXPORT_METHOD(restartInterface) {
    restartInterface();
}

RCT_EXPORT_METHOD(resetNetwork) {
    NSString* wifi_ipv4_addr = getWifiIpv4Addr();
    resetNetwork([wifi_ipv4_addr UTF8String]);
}

RCT_EXPORT_METHOD(start:(NSDictionary*)params)
{
  NSString* base_path = [params valueForKey:@"base_path"];
  NSString* non_addr = [params valueForKey:@"non_addr"];
  NSString* ws_addr = [params valueForKey:@"ws_addr"];
  NSNumber* bdt_port = [params valueForKey:@"bdt_port"];
  NSString* loglevel = [params valueForKey:@"loglevel"];
  
  NSString* wifi_ipv4_addr = getWifiIpv4Addr();
  
  dispatch_queue_t queue = dispatch_queue_create("rust_cocurrent_queue", DISPATCH_QUEUE_CONCURRENT);
  
  dispatch_async(queue, ^{
    NSLog(@"start cyfs stack in new thread.");
    start([base_path UTF8String], [non_addr UTF8String], [ws_addr UTF8String], [bdt_port intValue], [loglevel UTF8String], [wifi_ipv4_addr UTF8String], (void (*)(const uint8_t *))log_c);
  });
  /*
  [NSThread detachNewThreadWithBlock:^{
    NSLog(@"start cyfs stack in new thread.");
    start([base_path UTF8String], [non_addr UTF8String], [ws_addr UTF8String], [bdt_port intValue], [loglevel UTF8String], [wifi_ipv4_addr UTF8String], log_c);
  }];
   */
}

@end
