pragma solidity ^0.4.0;
    
contract DynamicArray {

    function create(uint n) {
        new uint[](n);
    }

    function createAndAccess(uint n) returns (uint) {
        uint[] memory tmp = new uint[](n);
        tmp[n - 1] = 7;
        return tmp[n - 1];
    }
}