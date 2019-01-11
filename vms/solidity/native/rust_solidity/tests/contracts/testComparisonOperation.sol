pragma solidity ^0.4.8;

contract Test {
    function f() returns (bool) {
        address a1 = 0x0102030405060708091011121314151617181920;
        address a2 = 0x2122232425262728293031323334353637383940;
        
        return a2 > a1;
    }
    
    function g() returns (bool) {
        address a1 = 0x0102030405060708091011121314151617181920;
        address a2 = 0x2122232425262728293031323334353637383940;
        
        return a1 < a2;
    }
}