pragma solidity ^0.4.15;

contract Test {
    function f(uint length) returns (uint[]) {
        uint[] memory r = new uint[](length);
        for (uint i = 0; i < r.length; i++) {
              r[i] = i;
        }
        return r;
      }
}
