pragma solidity ^0.4.0;

contract X {

   function add(uint a, uint b) returns (uint) {
        return a + b;
    }
}

contract Create {

   function f() returns (address) {
        return new X();
   }
}