pragma solidity ^0.4.10;

contract Math {

    function sum() returns (uint) {
        uint s = 0;
        for (uint i = 0; i <= 1024; i++) {
            s += i;
        }
        return s;
    }

    function fibonacci(uint n) returns(uint) {
        if (n <= 1) {
        	return n;
        } else {
        	return fibonacci(n - 1) + fibonacci(n - 2);
        }
    }
}
