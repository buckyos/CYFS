package com.cyfs;
import android.util.Log;

import com.facebook.react.bridge.WritableArray;
import com.facebook.react.bridge.WritableMap;
import com.facebook.react.bridge.Arguments;

public class Stack {

    static {
        System.loadLibrary("cyfsstack");
    }

    static boolean started = false;

    public static void client_start(String base_path, String non_addr, String ws_addr, int bdt_port, String loglevel, String wifi_addr) {
        if (started) {
            Log.e("CyfsStack", "recall client_start! ignore");
            return;
        }
        started = true;

        Thread clientThread = new Thread() {
            public void run() {
                Stack.start(base_path, non_addr, ws_addr, bdt_port, loglevel, wifi_addr);
            }
        };
        clientThread.start();
    }

    private static native void start(String base_path, String non_addr, String ws_addr, int bdt_port, String loglevel, String wifi_addr);
    public static native void resetNetwork(String addr);
    public static native void restartInterface();
}