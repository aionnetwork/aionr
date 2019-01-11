contract A {
   function A() {
       int sum = 0;
       for (int i = 0; i < 100000; i++) {
           sum += i;
       }
   }
}

contract Test {
   function f() {
       A a = new A();
   }
}
