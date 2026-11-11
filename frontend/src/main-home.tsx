import React from 'react';
import { createRoot } from 'react-dom/client';
import '../css/style.css';
import HomePage from './pages/HomePage';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <HomePage />
  </React.StrictMode>
);
