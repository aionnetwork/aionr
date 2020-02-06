pragma solidity ^0.4.10;

/**
 * Test array copy
 */
contract Test {
    uint8[] a;
    bytes32[] b;
    address[] c;

    function f() returns (uint8[]) {
        a.length = 2;
        a[0] = 0x1;
        a[1] = 0x2;
        
        uint8[] memory t = a;
        return t;
    }
    
    function g() returns (bytes32[]) {
        b.length = 2;
        b[0] = 0x1;
        b[1] = 0x2;
        
        bytes32[] memory t = b;
        return t;
    }
    
    function h() returns (address[]) {
        c.length = 2;
        c[0] = 0x0102030405060708091011121314151617181920;
        c[1] = 0x2122232425262728293031323334353637383940;
        
        address[] memory t = c;
        return t;
    }
}
