pragma solidity ^0.4.10;

contract Test {
   uint a = 0;

   function f() returns (uint) {
       a = 1;
       //this.call(bytes4(sha3("g()")));
       return a;
   }

   function g() {
       a = 2;
       revert();
   }
}
