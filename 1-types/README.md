# Assignment 1: Basic theory

## Is Rust single-threaded or multi-threaded? Is it synchronous or asynchronous?
Rust supports both single-threaded and multi-threaded programming.  
It is mainly synchronous, but also provides asynchronous programming using async/await.

## What runtime Rust has? Does it use a GC (garbage collector)?
Rust has a very minimal runtime and does not require a virtual machine.  
It does not use a garbage collector; memory management is done at compile time.

## What static typing means? What are the benefits of using it?
Static typing means that variable types are known at compile time.  
This helps catch many errors early and makes programs safer and faster.

## What is immutability? What is the benefit of using it?
Immutability means that a value cannot be changed after it is created.  
It helps prevent accidental bugs and makes code easier to reason about.

## What are move semantics? What are borrowing rules? What is the benefit of using them?
Move semantics transfer ownership of data instead of copying it.  
Borrowing rules ensure safe access to data without data races or memory errors.

## What are traits? How are they used? How do they compare to interfaces?
Traits define shared behavior that types can implement.  
They are similar to interfaces, but can also provide default method implementations.

## What are lifetimes? Which problems do they solve?
Lifetimes describe how long references are valid.  
They prevent dangling references and ensure memory safety.

## What are macros? Which problems do they solve?
Macros allow generating code at compile time.  
They help reduce repetitive code and enable more flexible abstractions.

## What is the difference between &String and &str types? Difference between fat and thin pointers?
&String is a reference to a String, while &str is a string slice.  
&str is a fat pointer because it stores both a pointer and a length.

## What are static and dynamic dispatches?
Static dispatch is resolved at compile time and is faster.  
Dynamic dispatch is resolved at runtime and allows more flexibility.
