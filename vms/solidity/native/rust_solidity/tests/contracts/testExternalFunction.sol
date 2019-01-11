pragma solidity ^0.4.10;

contract Test {
    function() external callback;
    
    function f() {
        callback = this.g;
    }
    
    function g() {
    }
}