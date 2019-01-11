pragma solidity >=0.4.10;

contract Test {
    function f() returns (bytes1) {
        bytes21 a = "test_very_long_string";

        return a[19];
    }

    function g() returns (bytes1) {
        bytes5 a = "short";

        return a[3];
    }
}
