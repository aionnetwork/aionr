pragma solidity ^0.4.8;

contract Test {
    function f() returns (bytes20) {
        bytes20 a1 = bytes20(0x0102030405060708091011121314151617181920);
        bytes20 a2 = bytes20(0x2122232425262728293031323334353637383940);
        
        return a1 & a2;
    }
    
    function g() returns (bytes20) {
        bytes20 a1 = bytes20(0x0102030405060708091011121314151617181920);
        bytes20 a2 = bytes20(0x2122232425262728293031323334353637383940);
        
        return a1 | a2;
    }
    
    function h() returns (bytes20) {
        bytes20 a1 = bytes20(0x0102030405060708091011121314151617181920);
        bytes20 a2 = bytes20(0x2122232425262728293031323334353637383940);
        
        return a1 & a2;
    }
}