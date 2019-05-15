package org.aion.avm.jni;

import java.io.ByteArrayOutputStream;
import java.io.IOException;

public class NativeEncoder {

    private ByteArrayOutputStream buffer;

    public NativeEncoder() {
        buffer = new ByteArrayOutputStream();
    }

    public void encodeByte(byte n) {
        buffer.write(0xff & n);
    }

    public void encodeShort(short n) {
        buffer.write(0xff & (n >> 8));
        buffer.write(0xff & n);
    }

    public void encodeInt(int n) {
        buffer.write(0xff & (n >> 24));
        buffer.write(0xff & (n >> 16));
        buffer.write(0xff & (n >> 8));
        buffer.write(0xff & n);
    }

    public void encodeLong(long n) {
        encodeInt((int) (n >> 32));
        encodeInt((int) (0xffffffff & n));
    }

    public void encodeBytes(byte[] bytes) {
        encodeInt(bytes.length);
        try {
            buffer.write(bytes);
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
    }

    public byte[] toByteArray() {
        return buffer.toByteArray();
    }
}
