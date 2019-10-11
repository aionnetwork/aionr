package org.aion.avm.jni;

public class NativeDecoder {

    private byte[] bytes;
    private int index;

    public NativeDecoder(byte[] bytes) {
        this.bytes = bytes;
    }

    public byte decodeByte() {
        require(1);
        return bytes[index++];
    }

    public short decodeShort() {
        require(2);
        short n = (short) (((0xff & bytes[index]) << 8) | (0xff & bytes[index + 1]));
        index += 2;
        return n;
    }

    public int decodeInt() {
        require(4);
        int n = ((0xff & bytes[index]) << 24) | ((0xff & bytes[index + 1]) << 16) | ((0xff & bytes[index + 2]) << 8) | (0xff & bytes[index + 3]);
        index += 4;
        return n;
    }

    public long decodeLong() {
        require(8);
        long n = ((0xffL & bytes[index]) << 56)
                | ((0xffL & bytes[index + 1]) << 48)
                | ((0xffL & bytes[index + 2]) << 40)
                | ((0xffL & bytes[index + 3]) << 32)
                | ((0xffL & bytes[index + 4]) << 24)
                | ((0xffL & bytes[index + 5]) << 16)
                | ((0xffL & bytes[index + 6]) << 8)
                | (0xffL & bytes[index + 7]);
        index += 8;
        return n;
    }

    public byte[] decodeBytes() {
        int size = decodeInt();
        require(size);
        byte[] tmp = new byte[size];
        System.arraycopy(bytes, index, tmp, 0, size);
        index += size;
        return tmp;
    }

    private void require(int n) {
        if (n < 0 || bytes.length - index < n) {
            throw new ArrayIndexOutOfBoundsException();
        }
    }
}
