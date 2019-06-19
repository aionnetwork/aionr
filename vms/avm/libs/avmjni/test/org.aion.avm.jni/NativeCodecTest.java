package org.aion.avm.jni;

import org.junit.Test;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;

public class NativeCodecTest {

    private byte b1 = Byte.MIN_VALUE;
    private byte b2 = Byte.MAX_VALUE;

    private short s1 = Short.MIN_VALUE;
    private short s2 = Short.MAX_VALUE;

    private int i1 = Integer.MIN_VALUE;
    private int i2 = Integer.MAX_VALUE;

    private long l1 = Long.MIN_VALUE;
    private long l2 = Long.MAX_VALUE;

    private byte[] bytes1 = new byte[0];
    private byte[] bytes2 = "hello".getBytes();
    private byte[] bytes3 = new byte[11111];

    @Test
    public void testCodec() {
        NativeEncoder enc = new NativeEncoder();
        enc.encodeByte(b1);
        enc.encodeByte(b2);
        enc.encodeShort(s1);
        enc.encodeShort(s2);
        enc.encodeInt(i1);
        enc.encodeInt(i2);
        enc.encodeLong(l1);
        enc.encodeLong(l2);
        enc.encodeBytes(bytes1);
        enc.encodeBytes(bytes2);
        enc.encodeBytes(bytes3);

        NativeDecoder dec = new NativeDecoder(enc.toByteArray());
        assertEquals(b1, dec.decodeByte());
        assertEquals(b2, dec.decodeByte());
        assertEquals(s1, dec.decodeShort());
        assertEquals(s2, dec.decodeShort());
        assertEquals(i1, dec.decodeInt());
        assertEquals(i2, dec.decodeInt());
        assertEquals(l1, dec.decodeLong());
        assertEquals(l2, dec.decodeLong());
        assertArrayEquals(bytes1, dec.decodeBytes());
        assertArrayEquals(bytes2, dec.decodeBytes());
        assertArrayEquals(bytes3, dec.decodeBytes());
    }
}
