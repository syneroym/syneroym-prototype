import type { Component } from 'solid-js';
import { Hello } from '@ui/solid';


const App: Component = () => {
  return (
    <>
      <p class="text-4xl text-green-700 text-center py-20">Hello tailwind!</p>
      <Hello />
    </>
  );
};

export default App;
