pragma solidity ^0.4.0;

contract Test {
    
    /* array push */
    function f() returns (uint) {
        uint[] x;
        x.push(0x11223344);
        return x[0];
    }
    
    /* bytearray push */
    function g() returns (byte) {
        bytes x;
        x.push(0x12);
        return x[0];
    }
}