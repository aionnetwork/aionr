pragma solidity >=0.4.10;

contract Test {
    function f(uint[] bits) returns (uint) {
        uint total = 0;
    
        for (uint i = 0; i < bits.length; i++) {
            total += bits[i];
        }

        return total;
    }
    
    function g(bytes32[] bits) returns (uint) {
        uint total = 0;

        for (uint i = 0; i < bits.length; i++) {
            address a = address(bytes20(bits[i]));
            uint value = uint(bytes12(bits[i] << 160));

            total += value;
        }

        return total;
    }
}
