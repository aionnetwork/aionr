pragma solidity >=0.4.10;

contract Test {
    function f() returns (string) {
        return "short_string";
    }
    
    function g() returns (string) {
        return "very_long_string_again_and_again_and_again_and_again_and_again_and_again_and_again_and_again";
    }
    
    function h() returns (string) {
        string memory a =  "123";
        
        return a;
    }
    
    function i() returns (bytes20) {
        return "a";
    }
}
