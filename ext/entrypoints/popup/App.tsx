import { useState } from 'react';
import './App.css';

function App() {
  const [count, setCount] = useState(0);
  const getTabDetail = async () => {
    const tab = await browser.tabs.query({ active: true, currentWindow: true });
    if (tab?.[0]) {
      const currentTab = tab[0];
      console.log('Tab Title:', currentTab.title);
      console.log('Tab URL:', currentTab.url);
    }
  };

  return (
    <div className="card">
      <button onClick={getTabDetail} type="button">
        Get current tab
      </button>
    </div>
  );
}

export default App;
