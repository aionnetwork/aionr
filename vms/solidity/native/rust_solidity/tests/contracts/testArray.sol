pragma solidity ^0.4.10;

contract Test {
    
    /*
     * Test resize dyanmic byte array
     */
    function f() returns (bytes) {
        bytes a;
        
        a.length = 40;
        
        for (uint i = 0; i < 40; i++) {
            a[i] = 'a';
        }
        
        return a;
    }
    
        
    /*
     * Test resize dyanmic byte array
     */
    function g() returns (bytes) {
        bytes a;
        
        a.length = 5;
        
        for (uint i = 0; i < 5; i++) {
            a[i] = 'a';
        }
        
        return a;
    }
    
    /**
     * Test access index
     */
     function h(address[] x) returns (address) {
         return x[2];
     }
}
