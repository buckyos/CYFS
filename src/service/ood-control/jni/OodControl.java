package com.cyfs;

public class OodControl {
    static {
        System.loadLibrary("ood_control");
    }

    public static void waitBind() {
        Thread clientThread = new Thread() {
            public void run() {
                OodControl.wait_bind();
            }
        };
        clientThread.start();
    }

    public static native void init(String base_path, String log_level);
    public static native void start();
    private static native void wait_bind();
    public static native boolean isBind();
    public static native String[] getAddressList();
}