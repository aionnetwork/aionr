pragma solidity ^0.4.0;

contract Fibonacci {

    function fibonacci(uint n) constant returns (uint) {
        if (n <= 1) {
            return n;
        } else {
            uint fib1 = 1;
            uint fib2 = 1;
            uint tmp = 0;
        
            for (uint i = 2; i < n; ++i) {
                tmp = fib1 + fib2;
                fib1 = fib2;
                fib2 = tmp;
            }
            return fib2;
        }
    }

    function fibonacciRecursive(uint n) constant returns (uint) {
        if (n <= 1) {
            return n;
        } else {
            return fibonacciRecursive(n - 1) + fibonacciRecursive(n - 2);
        }
    }

    function fibonacciArray(uint n) constant returns (uint) {
        if (n <= 1) {
            return n;
        } else {
            uint[] memory result = new uint[](n + 1);
            result[0] = 0;
            result[1] = 1;
            for (uint i = 2; i <= n; ++i) {
                result[i] = result[i - 1] + result[i - 2];
            }
            
            return result[n];
        }
    }

    function fibonacciSafe(uint n) constant returns (uint) {
        if (n > 50) {
            throw;
        }
        
        return fibonacci(n);
    }
}